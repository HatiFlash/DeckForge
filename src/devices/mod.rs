//! Catalogue experimental de peripheriques, sans ouverture HID ni changement de transport.
//! Sources de recherche : device-protocols/akp153.md, akp03.md,
//! mirajazz-readme.md, elgato-info.rs et elgato-lib.rs (non copies ici).
//! La reconnaissance ne garantit pas la compatibilite du protocole sur materiel.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family { Mirabox, Elgato }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat { Jpeg, Bmp }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol { MiraboxV1, MiraboxV2, MiraboxV3, ElgatoV1, ElgatoV2, ElgatoMini, ElgatoModule }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceModel {
    pub id: &'static str,
    pub name: &'static str,
    pub family: Family,
    pub vid: u16,
    pub pid: u16,
    pub rows: u8,
    pub cols: u8,
    pub keys: u8,
    pub image_w: u16,
    pub image_h: u16,
    pub image_format: ImageFormat,
    /// Rotation en degres (0, 90, 180, 270).
    pub rotation: u16,
    pub mirror_x: bool,
    pub mirror_y: bool,
    /// Taille nominale ; D6 lit la taille reelle du descripteur HID.
    pub packet_len: u16,
    pub protocol: Protocol,
    pub encoders: u8,
    /// Seul le D6 est valide par captures locales ; autres modeles experimentaux.
    pub verified: bool,
    pub notes: &'static str,
}

pub const MODELS: &[DeviceModel] = &[
    DeviceModel { id: "fifine_d6", name: "FIFINE D6", family: Family::Mirabox, vid: 0x3142, pid: 0x0060, rows: 3, cols: 5, keys: 15, image_w: 126, image_h: 126, image_format: ImageFormat::Jpeg, rotation: 180, mirror_x: true, mirror_y: true, packet_len: 512, protocol: Protocol::MiraboxV1, encoders: 0, verified: true, notes: "Taille de rapport mesuree sur le descripteur HID a la connexion ; 512 nominal." },
    DeviceModel { id: "akp153", name: "AJAZZ AKP153 / Mirabox HSV293S / 293V3", family: Family::Mirabox, vid: 0x5548, pid: 0x6674, rows: 3, cols: 5, keys: 15, image_w: 85, image_h: 85, image_format: ImageFormat::Jpeg, rotation: 90, mirror_x: true, mirror_y: true, packet_len: 512, protocol: Protocol::MiraboxV1, encoders: 0, verified: false, notes: "Trois marques pour le meme VID:PID ; modele experimental." },
    DeviceModel { id: "akp153e", name: "AJAZZ AKP153E", family: Family::Mirabox, vid: 0x0300, pid: 0x1010, rows: 3, cols: 5, keys: 15, image_w: 85, image_h: 85, image_format: ImageFormat::Jpeg, rotation: 90, mirror_x: true, mirror_y: true, packet_len: 512, protocol: Protocol::MiraboxV1, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "akp153r", name: "AJAZZ AKP153R", family: Family::Mirabox, vid: 0x0300, pid: 0x1020, rows: 3, cols: 5, keys: 15, image_w: 85, image_h: 85, image_format: ImageFormat::Jpeg, rotation: 90, mirror_x: true, mirror_y: true, packet_len: 512, protocol: Protocol::MiraboxV1, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "akp03", name: "AJAZZ AKP03", family: Family::Mirabox, vid: 0x0300, pid: 0x1001, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV2, encoders: 3, verified: false, notes: "Identifiant attribue a AKP153 dans un ancien registre contradictoire ; verifier sur materiel." },
    DeviceModel { id: "akp03e", name: "AJAZZ AKP03E", family: Family::Mirabox, vid: 0x0300, pid: 0x3002, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV3, encoders: 3, verified: false, notes: "" },
    DeviceModel { id: "akp03r", name: "AJAZZ AKP03R", family: Family::Mirabox, vid: 0x0300, pid: 0x1003, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV2, encoders: 3, verified: false, notes: "" },
    DeviceModel { id: "akp03r_rev2", name: "AJAZZ AKP03R rev. 2", family: Family::Mirabox, vid: 0x0300, pid: 0x3003, rows: 2, cols: 3, keys: 6, image_w: 64, image_h: 64, image_format: ImageFormat::Jpeg, rotation: 90, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV3, encoders: 3, verified: false, notes: "" },
    DeviceModel { id: "n3", name: "Mirabox N3", family: Family::Mirabox, vid: 0x6602, pid: 0x1002, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV2, encoders: 3, verified: false, notes: "" },
    DeviceModel { id: "n3_rev3", name: "Mirabox N3 rev. 3", family: Family::Mirabox, vid: 0x6603, pid: 0x1002, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV3, encoders: 3, verified: false, notes: "Revision selon VID ; a confirmer." },
    DeviceModel { id: "n3en", name: "Mirabox N3EN", family: Family::Mirabox, vid: 0x6603, pid: 0x1003, rows: 2, cols: 3, keys: 6, image_w: 60, image_h: 60, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::MiraboxV3, encoders: 3, verified: false, notes: "" },
    DeviceModel { id: "elgato_original", name: "Elgato Stream Deck Original", family: Family::Elgato, vid: 0x0fd9, pid: 0x0060, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Bmp, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 8191, protocol: Protocol::ElgatoV1, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_original_v2", name: "Elgato Stream Deck Original V2", family: Family::Elgato, vid: 0x0fd9, pid: 0x006d, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_mini", name: "Elgato Stream Deck Mini", family: Family::Elgato, vid: 0x0fd9, pid: 0x0063, rows: 2, cols: 3, keys: 6, image_w: 80, image_h: 80, image_format: ImageFormat::Bmp, rotation: 90, mirror_x: true, mirror_y: false, packet_len: 1024, protocol: Protocol::ElgatoMini, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_mini_mk2", name: "Elgato Stream Deck Mini Mk2", family: Family::Elgato, vid: 0x0fd9, pid: 0x0090, rows: 2, cols: 3, keys: 6, image_w: 80, image_h: 80, image_format: ImageFormat::Bmp, rotation: 90, mirror_x: true, mirror_y: false, packet_len: 1024, protocol: Protocol::ElgatoMini, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_mini_discord", name: "Elgato Stream Deck Mini Discord", family: Family::Elgato, vid: 0x0fd9, pid: 0x00b3, rows: 2, cols: 3, keys: 6, image_w: 80, image_h: 80, image_format: ImageFormat::Bmp, rotation: 90, mirror_x: true, mirror_y: false, packet_len: 1024, protocol: Protocol::ElgatoMini, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_mk2", name: "Elgato Stream Deck MK.2", family: Family::Elgato, vid: 0x0fd9, pid: 0x0080, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_mk2_scissor", name: "Elgato Stream Deck MK.2 scissor", family: Family::Elgato, vid: 0x0fd9, pid: 0x00a5, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_xl", name: "Elgato Stream Deck XL", family: Family::Elgato, vid: 0x0fd9, pid: 0x006c, rows: 4, cols: 8, keys: 32, image_w: 96, image_h: 96, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_xl_v2", name: "Elgato Stream Deck XL V2", family: Family::Elgato, vid: 0x0fd9, pid: 0x008f, rows: 4, cols: 8, keys: 32, image_w: 96, image_h: 96, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_neo", name: "Elgato Stream Deck Neo", family: Family::Elgato, vid: 0x0fd9, pid: 0x009a, rows: 2, cols: 4, keys: 8, image_w: 96, image_h: 96, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 0, verified: false, notes: "Bande LCD et touches tactiles hors perimetre M1." },
    DeviceModel { id: "elgato_plus", name: "Elgato Stream Deck Plus (touches)", family: Family::Elgato, vid: 0x0fd9, pid: 0x0084, rows: 2, cols: 4, keys: 8, image_w: 120, image_h: 120, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: false, mirror_y: false, packet_len: 1024, protocol: Protocol::ElgatoV2, encoders: 4, verified: false, notes: "Ecran tactile et commandes des molettes hors perimetre M1." },
    DeviceModel { id: "elgato_module_6", name: "Elgato Module 6", family: Family::Elgato, vid: 0x0fd9, pid: 0x00b8, rows: 2, cols: 3, keys: 6, image_w: 80, image_h: 80, image_format: ImageFormat::Bmp, rotation: 90, mirror_x: true, mirror_y: false, packet_len: 1024, protocol: Protocol::ElgatoModule, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_module_15", name: "Elgato Module 15", family: Family::Elgato, vid: 0x0fd9, pid: 0x00b9, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoModule, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_module_15_scissor", name: "Elgato Module 15 scissor", family: Family::Elgato, vid: 0x0fd9, pid: 0x00e4, rows: 3, cols: 5, keys: 15, image_w: 72, image_h: 72, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoModule, encoders: 0, verified: false, notes: "" },
    DeviceModel { id: "elgato_module_32", name: "Elgato Module 32", family: Family::Elgato, vid: 0x0fd9, pid: 0x00ba, rows: 4, cols: 8, keys: 32, image_w: 96, image_h: 96, image_format: ImageFormat::Jpeg, rotation: 0, mirror_x: true, mirror_y: true, packet_len: 1024, protocol: Protocol::ElgatoModule, encoders: 0, verified: false, notes: "" },
 ];

/// Descripteur HID reduit : construire depuis hidapi plus tard, sans ouvrir le materiel ici.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidInfo {
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub path: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    Matched { info: HidInfo, model: &'static DeviceModel },
    UnknownCandidate { info: HidInfo, reason: String },
}

pub fn detect(devices: &[HidInfo]) -> Vec<Detection> {
    devices.iter().filter_map(|info| {
        let known_vendor = matches!(info.vid, 0x0fd9 | 0x5548 | 0x0300 | 0x3142 | 0x6602 | 0x6603);
        if let Some(model) = MODELS.iter().find(|m| (m.vid, m.pid) == (info.vid, info.pid)) {
            let right_interface = match model.family {
                Family::Mirabox => info.usage_page >= 0xff00,
                Family::Elgato => info.interface == 0,
            };
            if right_interface { return Some(Detection::Matched { info: info.clone(), model }); }
            return Some(Detection::UnknownCandidate {
                info: info.clone(),
                reason: format!("Interface HID non retenue pour {} : usage_page=0x{:04x}, interface={}", model.name, info.usage_page, info.interface),
            });
        }
        known_vendor.then(|| Detection::UnknownCandidate {
            info: info.clone(),
            reason: format!("VID connu 0x{:04x}, PID inconnu 0x{:04x} : modele experimental a identifier", info.vid, info.pid),
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info(vid: u16, pid: u16, page: u16, interface: i32) -> HidInfo {
        HidInfo { vid, pid, usage_page: page, usage: 1, interface,
            product: None, manufacturer: None, path: "chemin fictif".into() }
    }
    #[test]
    fn table_is_consistent_and_unambiguous() {
        assert!(!MODELS.is_empty());
        for (i, m) in MODELS.iter().enumerate() {
            assert_eq!(m.keys as u16, m.rows as u16 * m.cols as u16, "{}", m.id);
            assert!(m.image_w > 0 && m.image_h > 0 && m.packet_len > 0);
            assert!([0, 90, 180, 270].contains(&m.rotation));
            assert!(MODELS[..i].iter().all(|other| other.id != m.id && (other.vid, other.pid) != (m.vid, m.pid)), "{}", m.id);
            assert_eq!(m.verified, m.id == "fifine_d6");
        }
    }
    #[test]
    fn d6_filter_is_identical_to_existing_device_code() {
        for page in [0, 0xfeff, 0xff00, 0xffff] {
            for (vid, pid) in [(0x3142, 0x0060), (0x3142, 0x0061), (0x0300, 0x1001)] {
                let dev = info(vid, pid, page, -1);
                let matched = detect(&[dev]).iter().any(|d| matches!(d, Detection::Matched { model, .. } if model.id == "fifine_d6"));
                assert_eq!(matched, vid == 0x3142 && pid == 0x0060 && page >= 0xff00);
            }
        }
    }
    #[test]
    fn unknown_pid_and_interface_are_candidates() {
        assert!(matches!(detect(&[info(0x0fd9, 0xdead, 0, 0)])[0], Detection::UnknownCandidate { .. }));
        assert!(matches!(detect(&[info(0x3142, 0x0060, 0, 0)])[0], Detection::UnknownCandidate { .. }));
        assert!(matches!(detect(&[info(0x0fd9, 0x0080, 0, 1)])[0], Detection::UnknownCandidate { .. }));
        assert!(detect(&[info(0x9999, 1, 0, 0)]).is_empty());
        assert!(matches!(detect(&[info(0x0fd9, 0x0080, 0, 0)])[0], Detection::Matched { .. }));
        assert!(matches!(detect(&[info(0x0300, 0x1001, 0xff00, 0)])[0], Detection::Matched { model, .. } if model.id == "akp03"));
    }
}
