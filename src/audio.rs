//! Windows Core Audio enumeration. COM pointers never leave their owning thread.
use anyhow::{Context, Result};
use std::{sync::mpsc::Sender, time::Duration};
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName, Media::Audio::*, System::Com::*,
    },
};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub input: bool,
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Snapshot {
    pub devices: Vec<Endpoint>,
    pub output: String,
    pub input: String,
    pub error: String,
}
struct Com;
impl Com {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
unsafe fn enumerator() -> Result<IMMDeviceEnumerator> {
    Ok(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?)
}
unsafe fn id(device: &IMMDevice) -> Result<String> {
    let value = device.GetId()?;
    let result = value.to_string();
    CoTaskMemFree(Some(value.0.cast()));
    Ok(result?)
}
pub fn snapshot() -> Result<Snapshot> {
    let _com = Com::new()?;
    unsafe {
        let e = enumerator()?;
        let mut s = Snapshot::default();
        for (input, flow) in [(false, eRender), (true, eCapture)] {
            if let Ok(d) = e.GetDefaultAudioEndpoint(flow, eConsole) {
                if input {
                    s.input = id(&d)?;
                } else {
                    s.output = id(&d)?;
                }
            }
            let devices = e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
            for i in 0..devices.GetCount()? {
                let d = devices.Item(i)?;
                let id = id(&d)?;
                let name = d
                    .OpenPropertyStore(STGM_READ)
                    .and_then(|p| p.GetValue(&PKEY_Device_FriendlyName))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| id.clone());
                s.devices.push(Endpoint { id, name, input });
            }
        }
        Ok(s)
    }
}
/// IPolicyConfig's stable Windows desktop ABI (not part of the public SDK).
/// Failures are surfaced to the user; no registry or elevated fallback is used.
pub fn set_default(target: &str, input: bool) -> Result<()> {
    let _com = Com::new()?;
    unsafe {
        let e = enumerator()?;
        let flow = if input { eCapture } else { eRender };
        let endpoints = e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        let mut found = false;
        for i in 0..endpoints.GetCount()? {
            found |= id(&endpoints.Item(i)?)? == target;
        }
        anyhow::ensure!(found, "Périphérique audio déconnecté ou indisponible");
        let cls = windows::core::GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
        let iid = windows::core::GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
        let mut raw = std::ptr::null_mut();
        CoCreateInstance::<_, windows::core::IUnknown>(&cls, None, CLSCTX_ALL)?
            .query(&iid, &mut raw)
            .ok()?;
        let policy = windows::core::IUnknown::from_raw(raw);
        let table = *(policy.as_raw() as *const *const usize);
        type Set = unsafe extern "system" fn(
            *mut std::ffi::c_void,
            PCWSTR,
            ERole,
        ) -> windows::core::HRESULT;
        let set: Set = std::mem::transmute(*table.add(13));
        let text: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        for role in [eConsole, eMultimedia, eCommunications] {
            set(policy.as_raw(), PCWSTR(text.as_ptr()), role)
                .ok()
                .context("Windows refuse le changement audio")?;
        }
        let active = id(&e.GetDefaultAudioEndpoint(flow, eConsole)?)?;
        anyhow::ensure!(
            active == target,
            "Windows n’a pas confirmé le changement audio"
        );
        Ok(())
    }
}
pub fn toggle(first: &str, second: &str, input: bool) -> Result<String> {
    let s = snapshot()?;
    let active = if input { s.input } else { s.output };
    let target = if active == first { second } else { first };
    set_default(target, input)?;
    Ok(target.into())
}
/// Explicit hardware verification: preserve every role, including a distinct
/// communications device. The drop guard restores settings on early errors.
pub fn verify_roundtrip() -> Result<serde_json::Value> {
    let _com = Com::new()?;
    unsafe {
        let e = enumerator()?;
        let mut before = vec![];
        for role in [eConsole, eMultimedia, eCommunications] {
            before.push((id(&e.GetDefaultAudioEndpoint(eRender, role)?)?, role));
        }
        struct Restore(Vec<(String, ERole)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (id, role) in &self.0 {
                    let _ = restore_role(id, *role);
                }
            }
        }
        let guard = Restore(before.clone());
        let state = snapshot()?;
        let target = state
            .devices
            .iter()
            .find(|d| !d.input && d.id != state.output && d.name.contains("Logitech"))
            .or_else(|| {
                state
                    .devices
                    .iter()
                    .find(|d| !d.input && d.id != state.output)
            })
            .context("Deux sorties audio actives sont nécessaires pour ce test")?;
        let forward = toggle(&state.output, &target.id, false)?;
        anyhow::ensure!(
            snapshot()?.output == target.id,
            "Le premier basculement n’a pas été confirmé"
        );
        let backward = toggle(&state.output, &target.id, false)?;
        anyhow::ensure!(
            snapshot()?.output == state.output,
            "Le retour audio n’a pas été confirmé"
        );
        for (id, role) in &before {
            restore_role(id, *role)?;
        }
        for (id_before, role) in &before {
            anyhow::ensure!(
                id(&e.GetDefaultAudioEndpoint(eRender, *role)?)? == *id_before,
                "Un rôle audio n’a pas été restauré"
            );
        }
        drop(guard);
        Ok(
            serde_json::json!({"passed":true,"forward":forward,"backward":backward,"all_roles_restored":true}),
        )
    }
}
fn restore_role(target: &str, role: ERole) -> Result<()> {
    unsafe {
        let cls = windows::core::GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
        let iid = windows::core::GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
        let mut raw = std::ptr::null_mut();
        CoCreateInstance::<_, windows::core::IUnknown>(&cls, None, CLSCTX_ALL)?
            .query(&iid, &mut raw)
            .ok()?;
        let policy = windows::core::IUnknown::from_raw(raw);
        let table = *(policy.as_raw() as *const *const usize);
        let set: unsafe extern "system" fn(
            *mut std::ffi::c_void,
            PCWSTR,
            ERole,
        ) -> windows::core::HRESULT = std::mem::transmute(*table.add(13));
        let text: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        set(policy.as_raw(), PCWSTR(text.as_ptr()), role).ok()?;
        Ok(())
    }
}
pub fn spawn(tx: Sender<crate::Event>) {
    std::thread::spawn(move || {
        let mut last = Snapshot::default();
        loop {
            let s = snapshot().unwrap_or_else(|e| Snapshot {
                error: e.to_string(),
                ..Default::default()
            });
            if s != last {
                last = s.clone();
                if tx.send(crate::Event::Audio(s)).is_err() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}
pub const BUILTIN_ICONS: &[(&str, &str, usize)] = &[
    ("builtin:headphones", "Casque", 26),
    ("builtin:speakers", "Enceintes", 27),
    ("builtin:microphone", "Microphone", 28),
];
pub fn builtin_kind(icon: &str) -> Option<usize> {
    BUILTIN_ICONS
        .iter()
        .find(|(id, _, _)| *id == icon)
        .map(|(_, _, kind)| *kind)
}
/// Feedback is unambiguous only when the macro contains one audio switch.
pub fn single_switch(steps: &[crate::model::Step]) -> Option<&crate::model::Step> {
    fn collect<'a>(steps: &'a [crate::model::Step], out: &mut Vec<&'a crate::model::Step>) {
        for step in steps {
            if out.len() > 1 {
                return;
            }
            match step {
                crate::model::Step::AudioSwitch { .. } => out.push(step),
                crate::model::Step::Repeat { steps, .. } => collect(steps, out),
                crate::model::Step::IfWindow {
                    then_steps,
                    else_steps,
                    ..
                }
                | crate::model::Step::IfVariable {
                    then_steps,
                    else_steps,
                    ..
                } => {
                    collect(then_steps, out);
                    collect(else_steps, out);
                }
                _ => {}
            }
        }
    }
    let mut found = vec![];
    collect(steps, &mut found);
    if found.len() == 1 {
        Some(found[0])
    } else {
        None
    }
}
pub fn present_key(key: &crate::model::Key, snapshot: &Snapshot) -> crate::model::Key {
    let mut shown = key.clone();
    if let Some(crate::model::Step::AudioSwitch {
        first,
        second,
        input,
        first_label,
        second_label,
        first_icon,
        second_icon,
    }) = single_switch(&key.steps)
    {
        let active = if *input {
            &snapshot.input
        } else {
            &snapshot.output
        };
        let (label, icon, kind) = if active == first {
            (first_label, first_icon, if *input { 28 } else { 26 })
        } else if active == second {
            (second_label, second_icon, if *input { 28 } else { 27 })
        } else {
            shown.image.clear();
            shown.display_kind = Some(if *input { 28 } else { 27 });
            if active.is_empty() {
                shown.label = "Audio indisponible".into();
            }
            return shown;
        };
        if !label.is_empty() {
            shown.label = label.clone();
        }
        // A static image on the key must never mask the audio state's artwork.
        shown.image = if icon.is_empty() || builtin_kind(icon).is_some() {
            String::new()
        } else {
            icon.clone()
        };
        shown.display_kind = Some(builtin_kind(icon).unwrap_or(kind));
    }
    shown
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Key, Step};
    pub(crate) fn key() -> Key {
        let mut k = crate::catalog::action(13, "Bureau");
        if let Step::AudioSwitch { first, second, .. } = &mut k.steps[0] {
            *first = "headset".into();
            *second = "speakers".into();
        }
        k.image = "old-static-image.png".into();
        k
    }
    #[test]
    fn audio_defaults_override_static_image_and_support_audio_after_a_delay() {
        let mut k = key();
        k.steps.insert(0, Step::Delay { ms: 10 });
        let a = present_key(
            &k,
            &Snapshot {
                output: "headset".into(),
                ..Default::default()
            },
        );
        let b = present_key(
            &k,
            &Snapshot {
                output: "speakers".into(),
                ..Default::default()
            },
        );
        assert!(a.image.is_empty() && b.image.is_empty());
        assert_eq!(a.display_kind, Some(26));
        assert_eq!(b.display_kind, Some(27));
        assert_eq!(k.image, "old-static-image.png");
    }
    #[test]
    fn builtin_and_file_choices_are_independent_for_each_state() {
        let mut k = key();
        if let Step::AudioSwitch {
            first_icon,
            second_icon,
            ..
        } = &mut k.steps[0]
        {
            *first_icon = "builtin:speakers".into();
            *second_icon = "custom-headphones.png".into();
        }
        let a = present_key(
            &k,
            &Snapshot {
                output: "headset".into(),
                ..Default::default()
            },
        );
        let b = present_key(
            &k,
            &Snapshot {
                output: "speakers".into(),
                ..Default::default()
            },
        );
        assert!(a.image.is_empty());
        assert_eq!(a.display_kind, Some(27));
        assert_eq!(b.image, "custom-headphones.png");
        let serialized = serde_json::to_vec(&k).unwrap();
        let restored: Key = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(
            present_key(
                &restored,
                &Snapshot {
                    output: "speakers".into(),
                    ..Default::default()
                }
            )
            .image,
            "custom-headphones.png"
        );
    }
    #[test]
    fn nested_single_switch_has_feedback_but_multiple_switches_are_not_guessed() {
        let mut k = key();
        k.steps = vec![Step::Repeat {
            count: 2,
            steps: k.steps,
        }];
        assert!(single_switch(&k.steps).is_some());
        let extra = key().steps.remove(0);
        k.steps.push(extra);
        assert!(single_switch(&k.steps).is_none());
    }
}
