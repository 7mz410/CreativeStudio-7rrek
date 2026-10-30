//! Encodes 12 frames of changing colour to MP4 and, when ffprobe is on PATH, checks the file it reads.

use std::process::Command;

use vectorcraft_video::Mp4Writer;

const W: u32 = 64;
const H: u32 = 48;
const FPS: u32 = 12;
const FRAMES: u32 = 12;

fn frame(k: u32, w: u32, h: u32) -> Vec<u8> {
    let r = (k * 20 % 256) as u8;
    let g = (255 - k * 20 % 256) as u8;
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        px.extend_from_slice(&[r, g, 128, 255]);
    }
    px
}

fn encode(w: u32, h: u32, frames: u32) -> Result<Vec<u8>, String> {
    let mut mp4 = Mp4Writer::new(w, h, FPS)?;
    for k in 0..frames {
        mp4.push(&frame(k, w, h))?;
    }
    mp4.finish()
}

#[test]
fn mp4_header_and_codec() -> Result<(), String> {
    let bytes = encode(W, H, FRAMES)?;
    assert_eq!(&bytes[4..8], b"ftyp", "ISO BMFF file type box");
    assert!(bytes.windows(4).any(|w| w == b"avc1"), "H.264 sample entry");
    assert!(bytes.windows(4).any(|w| w == b"avcC"), "H.264 decoder config");
    Ok(())
}

#[test]
fn odd_size_is_padded_to_even() -> Result<(), String> {
    let bytes = encode(63, 47, 3)?;
    assert_eq!(&bytes[4..8], b"ftyp");
    Ok(())
}

#[test]
fn refuses_empty_and_bad_input() {
    assert!(Mp4Writer::new(0, H, FPS).is_err());
    assert!(Mp4Writer::new(W, H, 0).is_err());
    assert!(Mp4Writer::new(W, H, FPS).and_then(|m| m.finish()).is_err(), "no frames");
    let mut mp4 = Mp4Writer::new(W, H, FPS).expect("writer");
    assert!(mp4.push(&[0u8; 3]).is_err(), "short frame");
}

#[test]
fn ffprobe_reads_it() -> Result<(), String> {
    if Command::new("ffprobe").arg("-version").output().is_err() {
        eprintln!("skipped: ffprobe not on PATH");
        return Ok(());
    }
    let bytes = encode(W, H, FRAMES)?;
    let path = std::env::temp_dir().join(format!("vectorcraft-video-{}.mp4", std::process::id()));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate,nb_read_frames",
            "-of",
            "csv=p=0",
        ])
        .arg(&path)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(line, "h264,64,48,12/1,12", "ffprobe: {} / stderr: {}", line, String::from_utf8_lossy(&out.stderr));
    Ok(())
}
