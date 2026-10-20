#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod assets;
mod assistant;
mod audio;
mod app_audio;
mod automation;
mod network_actions;
mod catalog;
mod conditions;
mod control;
mod desktop;
mod device;
#[allow(dead_code)] // Catalogue M1 : branchement HID dans un lot ulterieur.
mod devices;
mod elgato;
mod engine;
mod flicker_diagnostic;
mod gesture;
mod icons;
mod key_icons;
mod deck_chassis;
mod iconset;
mod motion;
mod key_render;
#[allow(dead_code)]
mod mcp_protocol;
mod mcp_server;
mod mcp_connect;
#[allow(dead_code)]
mod local_assistant;
mod local_llm;
mod model;
mod obs;
mod obs_monitor;
#[allow(dead_code)]
mod plugins;
#[allow(dead_code)]
mod plugin_host;
mod plugin_http;
mod plugin_inspector;
mod plugin_inspector_proxy;
mod plugin_runtime;
mod plugin_validation;
#[cfg(test)]
#[allow(dead_code)]
mod plugin_validation_proxy;
#[allow(dead_code)]
mod plugin_import;
mod plugin_catalog;
mod recorder;
mod soundboard;
mod store;
mod persistent_variables;
mod token_equal;
mod theme;
mod ui;
mod workshop_ops;
mod workshop_journal;
mod video_mf;
mod media_crop;
mod ui_media_crop;
mod state_display;

#[derive(Debug)]
pub enum Event {
    Log(String),
    Notification(String, String),
    Audio(audio::Snapshot),
    Block(Vec<usize>),
    WorkshopStep { path: Vec<usize>, name: String, elapsed: Option<std::time::Duration> },
    WorkshopFinished { ok: bool, elapsed: std::time::Duration, message: String },
    D6Brightness(String, u8),
    D6Sleep,
    ToggleVisual,
    StateDisplay(u64, usize, bool),
    Open,
    TrayReady(bool),
    Toggle,
    Quit,
    Hotkey(usize, std::time::Instant),
    Automatic(desktop::Trigger, std::time::Instant),
    Key(usize, std::time::Instant),
    KeyRelease(usize, std::time::Instant),
    Device(bool, String),
    HardwareTestPress(String, usize),
    HardwareTestStatus(String),
    DeviceModel { name: String, keys: u8, rows: u8, cols: u8, experimental: bool },
    Applied(u64),
    Profile(String),
    Foreground(Option<String>),
    Navigate(model::Step),
}

fn main() -> eframe::Result<()> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--isolated-inspector")) {
        if let Err(e) = plugin_inspector::run() {
            eprintln!("isolated inspector failed: {e:#}");
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--audio-roundtrip") {
        if let Some(p) = std::env::args()
            .skip_while(|a| a != "--audio-roundtrip")
            .nth(1)
        {
            let result = audio::verify_roundtrip().map_err(|e| e.to_string());
            let _ = std::fs::write(p, serde_json::to_vec_pretty(&result).unwrap());
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--audio-diagnostic") {
        if let Some(p) = std::env::args()
            .skip_while(|a| a != "--audio-diagnostic")
            .nth(1)
        {
            let data = audio::snapshot().map_err(|e| e.to_string());
            let _ = std::fs::write(p, serde_json::to_vec_pretty(&data).unwrap());
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--diagnostic") {
        device::diagnostic();
        return Ok(());
    }
    // One device owner per Windows session, including copies in other folders.
    let name = "Local\\DeckForgeD6\0".encode_utf16().collect::<Vec<_>>();
    let handle = unsafe {
        windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, name.as_ptr())
    };
    if handle.is_null() {
        return Ok(());
    }
    struct Instance(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Instance {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
    let _instance = Instance(handle);
    if unsafe { windows_sys::Win32::Foundation::GetLastError() }
        == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
    {
        if desktop::show_existing() {
            return Ok(());
        }
        let message = "DeckForge est déjà ouvert dans cette session.\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let title = "DeckForge\0".encode_utf16().collect::<Vec<_>>();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0,
            );
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--hardware-test") {
        device::hardware_test();
        return Ok(());
    }
    let size = if std::env::args().any(|a| a == "--small") {
        [1060.0, 720.0]
    } else {
        [1380.0, 900.0]
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DeckForge")
            .with_icon(theme::window_icon())
            .with_visible(!std::env::args().any(|a| a == "--background"))
            .with_inner_size(size)
            .with_min_inner_size([1000.0, 700.0]),
        // wgpu (DirectX 12/Vulkan) : presentation en mode flip, sans images vides au survol.
        renderer: if std::env::args().any(|a| a == "--glow") { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu },
        ..Default::default()
    };
    eframe::run_native(
        "DeckForge",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
