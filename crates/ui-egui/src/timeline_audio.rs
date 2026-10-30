//! 7rrek (Creative Studio): plays the timeline's sounds from the playhead through the default output
//! device while the animation plays. Native only: on the web both functions do nothing.
//!
//! Any failure (no device, an unreadable file) is logged and the sound stays silent; the animation
//! itself never depends on this.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use vectorcraft_audio::{Clip, Pcm};
    use vectorcraft_doc::Document;
    use vectorcraft_engine::cmd::anim::{Anim, Sound};

    use crate::VectorcraftApp;

    thread_local! {
        /// Each sound file's decoded audio, by path; `None` when it can't be read or decoded.
        static PCM: RefCell<HashMap<String, Option<Rc<Pcm>>>> = RefCell::new(HashMap::new());
        /// The stream that is playing now. Dropping it stops the sound.
        static STREAM: RefCell<Option<cpal::Stream>> = const { RefCell::new(None) };
    }

    /// A sound's decoded audio (its embedded bytes, else its file), decoded on first use only.
    fn pcm(doc: &Document, s: &Sound) -> Option<Rc<Pcm>> {
        let key = s.data.clone().unwrap_or_else(|| s.path.clone());
        PCM.with(|c| {
            if let Some(v) = c.borrow().get(&key) {
                return v.clone();
            }
            // shortcut: decodes on the UI thread at the first play, as timeline_av's peaks do; move
            // it to a worker when long sound files make the first press stutter.
            let v = load(doc, s).map(Rc::new);
            c.borrow_mut().insert(key, v.clone());
            v
        })
    }

    fn load(doc: &Document, s: &Sound) -> Option<Pcm> {
        let bytes = vectorcraft_engine::cmd::anim_sound::sound_bytes(doc, s)?;
        let ext = std::path::Path::new(&s.path).extension().and_then(|e| e.to_str());
        match vectorcraft_audio::decode(&bytes, ext) {
            Ok(p) => Some(p),
            Err(e) => {
                log::warn!("timeline sound {}: {e}", s.name);
                None
            }
        }
    }

    /// The timeline from `from_frame` (1-based) to the end, as interleaved stereo at `rate` Hz.
    /// `sounds` are `(start frame, gain, decoded audio)`; a sound's start is a frame, so its time is
    /// `(start - 1) / fps` seconds.
    pub fn mix_from(sounds: &[(u32, f32, &Pcm)], fps: u32, frames: u32, from_frame: u32, rate: u32) -> Vec<f32> {
        if fps == 0 {
            return Vec::new();
        }
        let f = f64::from(fps);
        let clips: Vec<Clip> = sounds.iter().map(|&(start, gain, pcm)| Clip { pcm, start: f64::from(start.max(1) - 1) / f, gain }).collect();
        let from = f64::from(from_frame.max(1) - 1) / f;
        let to = f64::from(frames) / f;
        let mixed = vectorcraft_audio::mix(&clips, from, to);
        resample(&mixed, vectorcraft_audio::RATE, rate)
    }

    /// Linear-interpolation resampling of interleaved stereo.
    fn resample(src: &[f32], from: u32, to: u32) -> Vec<f32> {
        if from == to || from == 0 || to == 0 || src.len() < 2 {
            return src.to_vec();
        }
        // shortcut: linear interpolation with no low-pass filter; aliasing is faint for sound
        // effects. Upgrade to a band-limited (sinc) resampler if downsampled audio sounds harsh.
        let frames = src.len() / 2;
        let out_frames = ((frames as u64 * u64::from(to) + u64::from(from) / 2) / u64::from(from)) as usize;
        let step = f64::from(from) / f64::from(to);
        let last = frames - 1;
        let mut out = Vec::with_capacity(out_frames * 2);
        for i in 0..out_frames {
            let pos = i as f64 * step;
            let i0 = (pos.floor() as usize).min(last);
            let i1 = (i0 + 1).min(last);
            let t = (pos - i0 as f64).clamp(0.0, 1.0) as f32;
            for c in 0..2 {
                let a = src.get(i0 * 2 + c).copied().unwrap_or(0.0);
                let b = src.get(i1 * 2 + c).copied().unwrap_or(0.0);
                out.push(a + (b - a) * t);
            }
        }
        out
    }

    /// An output stream of sample type `T` that plays `buf` once, then stays silent.
    fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, buf: Vec<f32>) -> Result<cpal::Stream, String>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let ch = usize::from(config.channels.max(1));
        let mut pos = 0usize;
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(ch) {
                        let l = buf.get(pos).copied().unwrap_or(0.0);
                        let r = buf.get(pos + 1).copied().unwrap_or(0.0);
                        pos = pos.saturating_add(2);
                        for (c, s) in frame.iter_mut().enumerate() {
                            let v = match (ch, c) {
                                (1, _) => (l + r) * 0.5,
                                (_, 0) => l,
                                (_, 1) => r,
                                _ => 0.0,
                            };
                            *s = T::from_sample(v);
                        }
                    }
                },
                |e| log::warn!("timeline sound output: {e}"),
                None,
            )
            .map_err(|e| e.to_string())
    }

    pub fn stop() {
        STREAM.with(|s| s.borrow_mut().take());
    }

    pub fn start(app: &VectorcraftApp, from_frame: u32) {
        stop();
        let Some(st) = app.session.active() else { return };
        let a = Anim::of(&st.doc);
        let clips: Vec<(u32, f32, Rc<Pcm>)> = a.sounds.iter().filter_map(|s| Some((s.start, s.gain, pcm(&st.doc, s)?))).collect();
        if clips.is_empty() {
            return;
        }
        if let Err(e) = play(&clips, a.fps, a.frames, from_frame) {
            log::warn!("timeline sound: {e}");
        }
    }

    fn play(clips: &[(u32, f32, Rc<Pcm>)], fps: u32, frames: u32, from_frame: u32) -> Result<(), String> {
        let device = cpal::default_host().default_output_device().ok_or("no output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = supported.sample_rate().0;
        let refs: Vec<(u32, f32, &Pcm)> = clips.iter().map(|(s, g, p)| (*s, *g, p.as_ref())).collect();
        let buf = mix_from(&refs, fps, frames, from_frame, rate);
        let config = supported.config();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, buf)?,
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, buf)?,
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, buf)?,
            other => return Err(format!("unsupported sample format {other:?}")),
        };
        stream.play().map_err(|e| e.to_string())?;
        STREAM.with(|c| *c.borrow_mut() = Some(stream));
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{Pcm, mix_from, resample};

        /// `frames` stereo frames of a constant level.
        fn flat(frames: usize, v: f32) -> Pcm {
            Pcm { samples: vec![v; frames * 2] }
        }

        /// 10 fps over 30 frames is 3 s. A 0.1 s sound starts on frame 3, at 0.2 s.
        #[test]
        fn from_frame_puts_each_sound_at_its_offset() {
            let pcm = flat(4800, 0.5);
            // Playing from frame 2 (0.1 s): the sound starts 0.1 s into the window, 4800 frames
            // in, which is interleaved sample 9600.
            let out = mix_from(&[(3, 1.0, &pcm)], 10, 30, 2, 48_000);
            assert_eq!(out.len(), 139_200 * 2);
            assert_eq!(out[9_598], 0.0);
            assert_eq!(out[9_600], 0.5);
            assert_eq!(out[9_601], 0.5);
            // From frame 3 the sound starts at the window's first frame.
            let out = mix_from(&[(3, 1.0, &pcm)], 10, 30, 3, 48_000);
            assert_eq!(out[0], 0.5);
        }

        #[test]
        fn resampling_gives_the_right_length_and_position() {
            let pcm = flat(4800, 0.5);
            let out = mix_from(&[(3, 1.0, &pcm)], 10, 30, 2, 44_100);
            // 139200 frames at 48 kHz are 127890 frames at 44.1 kHz (rounded).
            assert_eq!(out.len(), 127_890 * 2);
            // 4800 source frames at 48 kHz are 4410 at 44.1 kHz: the sound starts at output frame 4410.
            assert_eq!(out[4_410 * 2], 0.5);
            assert_eq!(out[4_409 * 2], 0.0);
        }

        #[test]
        fn same_rate_is_untouched() {
            let src = vec![0.1, -0.1, 0.2, -0.2];
            assert_eq!(resample(&src, 48_000, 48_000), src);
        }

        #[test]
        fn no_fps_or_no_window_gives_nothing() {
            let pcm = flat(10, 0.5);
            assert!(mix_from(&[(1, 1.0, &pcm)], 0, 30, 1, 48_000).is_empty());
            assert!(mix_from(&[(1, 1.0, &pcm)], 10, 30, 31, 48_000).is_empty());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{start, stop};

/// The web build has no audio output here: both calls do nothing.
#[cfg(target_arch = "wasm32")]
pub fn start(_app: &crate::VectorcraftApp, _from_frame: u32) {}

#[cfg(target_arch = "wasm32")]
pub fn stop() {}
