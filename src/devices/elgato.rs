//! Encodage Elgato expérimental : fonctions pures, sans ouverture HID.
use super::{DeviceModel, ImageFormat, Protocol};
use anyhow::{ensure, Result};

fn legacy(model: &DeviceModel) -> bool {
    model.protocol == Protocol::ElgatoV1 || model.image_format == ImageFormat::Bmp
}

pub fn brightness(model: &DeviceModel, percent: u8) -> Vec<u8> {
    if legacy(model) {
        let mut data = vec![5, 0x55, 0xaa, 0xd1, 1, percent.min(100)];
        data.resize(17, 0); data
    } else {
        let mut data = vec![3, 8, percent.min(100)];
        data.resize(32, 0); data
    }
}
pub fn reset(model: &DeviceModel) -> Vec<u8> {
    let mut data = if legacy(model) { vec![11, 0x63] } else { vec![3, 2] };
    data.resize(if legacy(model) { 17 } else { 32 }, 0); data
}
pub fn firmware_layout(model: &DeviceModel) -> (u8, usize, usize) {
    if model.id == "elgato_module_6" { (0xa1, 17, 5) }
    else if legacy(model) { (4, 17, 5) } else { (5, 32, 6) }
}
pub fn firmware(model: &DeviceModel, data: &[u8]) -> Option<String> {
    let (id, _, offset) = firmware_layout(model);
    if data.first() != Some(&id) || data.len() <= offset { return None; }
    let bytes = data[offset..].split(|b| *b == 0).next()?;
    (!bytes.is_empty()).then(|| String::from_utf8_lossy(bytes).to_string())
}

/// Indices logiques de gauche à droite ; l'Original numérote les images depuis la droite.
pub fn image_key(model: &DeviceModel, index: usize) -> Result<u8> {
    ensure!(index < model.keys as usize, "Touche Elgato hors grille : {index}");
    if model.protocol == Protocol::ElgatoV1 {
        Ok((index / model.cols as usize * model.cols as usize + model.cols as usize - 1 - index % model.cols as usize) as u8 + 1)
    } else if legacy(model) { Ok(index as u8 + 1) } else { Ok(index as u8) }
}

/// Rapport complet, rempli de zéros. Original : exactement deux pages BMP.
pub fn image_reports(model: &DeviceModel, index: usize, data: &[u8]) -> Result<Vec<Vec<u8>>> {
    ensure!(!data.is_empty(), "Image Elgato vide");
    let key = image_key(model, index)?;
    let original = model.protocol == Protocol::ElgatoV1;
    let mini = legacy(model) && !original;
    let size = model.packet_len as usize;
    let header = if original || mini { 16 } else { 8 };
    ensure!(size > header, "Rapport Elgato trop court");
    let payload = if original { data.len().div_ceil(2) } else { size - header };
    ensure!(payload <= size - header, "BMP Original trop lourd pour deux rapports");
    let count = data.len().div_ceil(payload);
    ensure!(count <= u16::MAX as usize, "Trop de pages image Elgato");
    let mut reports = Vec::with_capacity(count);
    for (seq, chunk) in data.chunks(payload).enumerate() {
        let mut report = vec![0u8; size];
        if original || mini {
            ensure!(seq < 256, "Séquence BMP trop longue");
            report[..6].copy_from_slice(&[2, 1, (seq + usize::from(original)) as u8, 0, u8::from(seq + 1 == count), key]);
        } else {
            report[..8].copy_from_slice(&[2, 7, key, u8::from(seq + 1 == count), chunk.len() as u8, (chunk.len() >> 8) as u8, seq as u8, (seq >> 8) as u8]);
        }
        report[header..header + chunk.len()].copy_from_slice(chunk);
        reports.push(report);
    }
    Ok(reports)
}

/// None : rapport non-touché (écran/molettes Plus) ou tronqué.
pub fn keys(model: &DeviceModel, data: &[u8]) -> Option<Vec<(usize, bool)>> {
    let plus = model.id == "elgato_plus";
    if plus && data.get(1) != Some(&0) { return None; }
    let offset = if plus { 4 } else if legacy(model) { 1 } else { 4 };
    if data.first() == Some(&0) || data.len() < offset + model.keys as usize { return None; }
    let cols = model.cols as usize;
    Some((0..model.keys as usize).map(|i| {
        // The Original reports its keys right-to-left on each row (confirmed on hardware:
        // the top-left key arrives at raw position 5). Same mapping as image_key().
        let logical = if model.protocol == Protocol::ElgatoV1 { i / cols * cols + cols - 1 - i % cols } else { i };
        (logical, data[offset + i] != 0)
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::MODELS;
    fn m(id: &str) -> &'static DeviceModel { MODELS.iter().find(|m| m.id == id).unwrap() }
    /// Raw reports captured on a real Stream Deck Original (firmware 1.0.191203).
    #[test] fn original_keys_are_read_right_to_left_like_real_hardware() {
        let original = m("elgato_original");
        let pressed = |raw_pos: usize| { let mut d = vec![0u8; 17]; d[0] = 1; d[raw_pos] = 1; keys(original, &d).unwrap().into_iter().find(|(_, down)| *down).unwrap().0 };
        assert_eq!([pressed(5), pressed(4), pressed(3), pressed(2), pressed(1)], [0, 1, 2, 3, 4], "top row, left to right");
        assert_eq!(pressed(10), 5); assert_eq!(pressed(6), 9);
        assert_eq!(pressed(15), 10); assert_eq!(pressed(11), 14);
        for logical in 0..15 { let raw = image_key(original, logical).unwrap() as usize; assert_eq!(pressed(raw), logical, "image and input use the same position"); }
    }
    #[test] fn images_v1_mini_v2_and_modules() {
        let original = m("elgato_original");
        assert_eq!(image_key(original, 0).unwrap(), 5);
        assert_eq!(image_key(original, 5).unwrap(), 10);
        let reports = image_reports(original, 0, &vec![0x42; 15590]).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(&reports[0][..6], &[2,1,1,0,0,5]);
        assert_eq!(&reports[1][..6], &[2,1,2,0,1,5]);
        assert_eq!(reports[0].len(), 8191);
        assert!(image_reports(original, 15, &[1]).is_err());
        let mini = image_reports(m("elgato_mini"), 1, &vec![9; 1020]).unwrap();
        assert_eq!(&mini[0][..6], &[2,1,0,0,0,2]);
        assert_eq!(&mini[1][..6], &[2,1,1,0,1,2]);
        for id in ["elgato_mk2", "elgato_module_15", "elgato_module_32", "elgato_plus"] {
            let pages = image_reports(m(id), 1, &vec![7; 1020]).unwrap();
            assert_eq!(&pages[0][..8], &[2,7,1,0,0xf8,3,0,0]);
            assert_eq!(&pages[1][..8], &[2,7,1,1,4,0,1,0]);
        }
        let module = image_reports(m("elgato_module_6"), 0, &[1]).unwrap();
        assert_eq!(&module[0][..6], &[2,1,0,0,1,1]);
    }
    #[test] fn features_and_input() {
        assert_eq!(&brightness(m("elgato_original"), 120)[..6], &[5,0x55,0xaa,0xd1,1,100]);
        assert_eq!(&brightness(m("elgato_plus"), 40)[..3], &[3,8,40]);
        assert_eq!(&reset(m("elgato_mini"))[..2], &[11,0x63]);
        assert_eq!(&reset(m("elgato_mk2"))[..2], &[3,2]);
        assert_eq!(firmware_layout(m("elgato_module_6")), (0xa1,17,5));
        let mut report = vec![0;17]; report[0]=4; report[5..8].copy_from_slice(b"1.2");
        assert_eq!(firmware(m("elgato_mini"), &report).as_deref(), Some("1.2"));
        assert!(firmware(m("elgato_mk2"), &report).is_none());
        let mut input = vec![0;16]; input[0]=1; input[1]=1;
        // Raw position 1 is the top-RIGHT key on a real Original (logical index 4).
        assert_eq!(keys(m("elgato_original"), &input).unwrap()[0], (4,true));
        let mut v2 = vec![0;19]; v2[0]=1; v2[4]=1;
        assert_eq!(keys(m("elgato_mk2"), &v2).unwrap()[0], (0,true));
        let mut plus = vec![0;12]; plus[0]=1; plus[1]=0; plus[4]=1;
        assert_eq!(keys(m("elgato_plus"), &plus).unwrap()[0], (0,true));
        plus[1]=3; assert!(keys(m("elgato_plus"), &plus).is_none());
        assert!(keys(m("elgato_mini"), &[1,0]).is_none());
        assert!(MODELS.iter().filter(|m| m.family == crate::devices::Family::Elgato).all(|m| !m.verified));
    }
}
