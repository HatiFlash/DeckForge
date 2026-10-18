//! Native video decoding with Windows Media Foundation, so common videos (MP4/MOV
//! H.264) can be imported without FFmpeg. Only the first `max_secs` seconds are read.
use anyhow::{anyhow, ensure, Result};
use image::{imageops, RgbaImage};
use std::path::Path;
use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const ALL: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;

/// Square `size`×`size` frames sampled at `fps`, cropped like the FFmpeg path (cover + centre crop).
pub fn decode(path: &Path, fps: u32, max_secs: u32, size: u32) -> Result<Vec<RgbaImage>> {
    ensure!(fps > 0 && size > 0, "Paramètres vidéo invalides");
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let started = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        let result = match started {
            Ok(()) => { let r = read(path, fps, max_secs, size); let _ = MFShutdown(); r }
            Err(e) => Err(anyhow!("Media Foundation indisponible ({e}) : édition N de Windows sans Media Feature Pack ?")),
        };
        if com {
            CoUninitialize();
        }
        result
    }
}

unsafe fn read(path: &Path, fps: u32, max_secs: u32, size: u32) -> Result<Vec<RgbaImage>> {
    let mut attributes = None;
    MFCreateAttributes(&mut attributes, 1)?;
    let attributes = attributes.ok_or_else(|| anyhow!("Attributs Media Foundation absents"))?;
    attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
    let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)
        .map_err(|e| anyhow!("Windows ne sait pas ouvrir cette vidéo ({})", e.message()))?;
    reader.SetStreamSelection(ALL, false)?;
    reader
        .SetStreamSelection(VIDEO, true)
        .map_err(|_| anyhow!("Aucune piste vidéo dans ce fichier"))?;
    let wanted = MFCreateMediaType()?;
    wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
    reader
        .SetCurrentMediaType(VIDEO, None, &wanted)
        .map_err(|e| {
            anyhow!(
                "Codec vidéo non pris en charge par Windows ({})",
                e.message()
            )
        })?;
    let current = reader.GetCurrentMediaType(VIDEO)?;
    let packed = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
    let (width, height) = ((packed >> 32) as u32, packed as u32);
    ensure!(
        width > 0 && height > 0 && width <= 8192 && height <= 8192,
        "Taille de vidéo invalide ({width}×{height})"
    );
    let stride = current
        .GetUINT32(&MF_MT_DEFAULT_STRIDE)
        .map(|s| s as i32)
        .unwrap_or((width * 4) as i32);
    let step = 10_000_000i64 / fps as i64;
    let limit = max_secs as i64 * 10_000_000;
    let mut next = 0i64;
    let mut frames = Vec::new();
    loop {
        let (mut flags, mut stamp, mut sample) = (0u32, 0i64, None::<IMFSample>);
        reader.ReadSample(
            VIDEO,
            0,
            None,
            Some(&mut flags),
            Some(&mut stamp),
            Some(&mut sample),
        )?;
        if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
            anyhow::bail!("Erreur de décodage vidéo");
        }
        if stamp >= limit || frames.len() as u32 >= fps * max_secs {
            break;
        }
        if let Some(sample) = sample {
            if stamp >= next {
                frames.push(to_square(&sample, width, height, stride, size)?);
                while next <= stamp {
                    next += step;
                }
            }
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
    }
    ensure!(!frames.is_empty(), "Aucune image lisible dans cette vidéo");
    Ok(frames)
}

unsafe fn to_square(
    sample: &IMFSample,
    width: u32,
    height: u32,
    stride: i32,
    size: u32,
) -> Result<RgbaImage> {
    let buffer = sample.ConvertToContiguousBuffer()?;
    let mut data = std::ptr::null_mut();
    let mut length = 0u32;
    buffer.Lock(&mut data, None, Some(&mut length))?;
    let bytes = std::slice::from_raw_parts(data, length as usize);
    let image = bgra_to_rgba(bytes, width, height, stride);
    let _ = buffer.Unlock();
    Ok(cover(&image?, size))
}

/// Converts Media Foundation RGB32 (BGRX, top-down if stride > 0) to opaque RGBA.
pub(crate) fn bgra_to_rgba(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: i32,
) -> Result<RgbaImage> {
    let row = stride.unsigned_abs() as usize;
    ensure!(
        row >= width as usize * 4 && bytes.len() >= row * height as usize,
        "Image vidéo tronquée"
    );
    let mut out = RgbaImage::new(width, height);
    for y in 0..height {
        let source = if stride >= 0 { y } else { height - 1 - y } as usize * row;
        for x in 0..width {
            let p = source + x as usize * 4;
            out.put_pixel(
                x,
                y,
                image::Rgba([bytes[p + 2], bytes[p + 1], bytes[p], 255]),
            );
        }
    }
    Ok(out)
}

/// Scale to cover a square then crop the centre (same framing as the FFmpeg filter).
pub(crate) fn cover(image: &RgbaImage, size: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    let scale = size as f32 / w.min(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(size),
        ((h as f32 * scale).round() as u32).max(size),
    );
    let scaled = imageops::resize(image, nw, nh, imageops::FilterType::Triangle);
    imageops::crop_imm(&scaled, (nw - size) / 2, (nh - size) / 2, size, size).to_image()
}

/// Encodes frames as an infinitely looping GIF.
pub fn encode_gif(frames: Vec<RgbaImage>, fps: u32) -> Result<Vec<u8>> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new_with_speed(&mut bytes, 10);
        encoder.set_repeat(Repeat::Infinite)?;
        let delay = image::Delay::from_numer_denom_ms(1000, fps.max(1));
        encoder.encode_frames(
            frames
                .into_iter()
                .map(|f| image::Frame::from_parts(f, 0, 0, delay)),
        )?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_up_bgra_and_centre_crop() {
        // 2×2 BGRX, bottom-up: first row in memory is the bottom row.
        let bytes = [0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        let image = bgra_to_rgba(&bytes, 2, 2, -8).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [255, 0, 0, 255]);
        let wide = RgbaImage::from_pixel(200, 100, image::Rgba([1, 2, 3, 255]));
        assert_eq!(cover(&wide, 72).dimensions(), (72, 72));
        assert!(bgra_to_rgba(&bytes[..8], 2, 2, 8).is_err());
        let gif = encode_gif(vec![cover(&wide, 72), cover(&wide, 72)], 12).unwrap();
        assert!(gif.starts_with(b"GIF89a"));
    }
}
