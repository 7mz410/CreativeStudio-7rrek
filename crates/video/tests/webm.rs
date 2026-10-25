//! Encodes 12 frames of changing colour and, when ffprobe is on PATH, checks the file it reads.

use std::process::Command;

use vectorcraft_video::{WebmWriter, encode_webm};

const W: u32 = 64;
const H: u32 = 48;
const FPS: u32 = 12;
const FRAMES: u32 = 12;

fn frame(k: u32) -> Vec<u8> {
    let r = (k * 20 % 256) as u8;
    let g = (255 - k * 20 % 256) as u8;
    let mut px = Vec::with_capacity((W * H * 4) as usize);
    for _ in 0..W * H {
        px.extend_from_slice(&[r, g, 128, 255]);
    }
    px
}

#[test]
fn webm_header_and_frame_count() -> Result<(), String> {
    let bytes = encode_webm((0..FRAMES).map(frame), W, H, FPS)?;
    assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "EBML magic");
    assert!(bytes.windows(5).any(|w| w == b"V_VP9"), "codec id");
    Ok(())
}

#[test]
fn streams_odd_size() -> Result<(), String> {
    let mut w = WebmWriter::new(63, 47, 25)?;
    for _ in 0..3 {
        w.push(&vec![90u8; 63 * 47 * 4])?;
    }
    assert_eq!(w.frame_count(), 3);
    assert_eq!(&w.finish()[..4], &[0x1A, 0x45, 0xDF, 0xA3]);
    Ok(())
}

#[test]
fn refuses_empty_and_bad_sizes() {
    assert!(encode_webm(Vec::<Vec<u8>>::new(), W, H, FPS).is_err());
    assert!(encode_webm([frame(0)], 0, H, FPS).is_err());
    assert!(encode_webm([frame(0)], W, H, 0).is_err());
    assert!(encode_webm([vec![0u8; 3]], W, H, FPS).is_err());
}

#[test]
fn ffprobe_reads_it() -> Result<(), String> {
    if Command::new("ffprobe").arg("-version").output().is_err() {
        eprintln!("skipped: ffprobe not on PATH");
        return Ok(());
    }
    let bytes = encode_webm((0..FRAMES).map(frame), W, H, FPS)?;
    let path = std::env::temp_dir().join(format!("vectorcraft-video-{}.webm", std::process::id()));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames,width,height,r_frame_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(&path)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    eprintln!("ffprobe: {line}");
    let fields: Vec<&str> = line.split(',').collect();
    for want in ["64", "48", "12/1", "12"] {
        assert!(fields.contains(&want), "ffprobe line {line:?} lacks {want}");
    }
    Ok(())
}
