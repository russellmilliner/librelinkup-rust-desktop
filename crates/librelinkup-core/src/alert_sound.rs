//! Built-in default alarm sound.
//!
//! The previous default (`QApplication::beep()`) is a no-op on most Plasma setups, so
//! out-of-range glucose alerts were effectively silent. Instead of shipping a binary
//! asset, a short, attention-grabbing WAV (two bursts of three 880 Hz beeps) is
//! synthesized once into the app data directory and played with `pw-play`/`paplay`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::paths::default_alert_sound_path;

pub const SAMPLE_RATE: u32 = 44_100;
const TONE_HZ: f64 = 880.0;
const BEEP_SECS: f64 = 0.16;
const GAP_SECS: f64 = 0.08;
const BURST_GAP_SECS: f64 = 0.35;
const BEEPS_PER_BURST: usize = 3;
const BURSTS: usize = 2;
const AMPLITUDE: f64 = 0.6;
const FADE_SECS: f64 = 0.008;

/// Generates mono 16-bit PCM samples for the alarm pattern.
pub fn generate_alert_samples() -> Vec<i16> {
    let beep_len = (BEEP_SECS * SAMPLE_RATE as f64) as usize;
    let gap_len = (GAP_SECS * SAMPLE_RATE as f64) as usize;
    let burst_gap_len = (BURST_GAP_SECS * SAMPLE_RATE as f64) as usize;
    let fade_len = ((FADE_SECS * SAMPLE_RATE as f64) as usize).max(1);

    let mut samples = Vec::new();
    for burst in 0..BURSTS {
        for beep in 0..BEEPS_PER_BURST {
            for i in 0..beep_len {
                // Short linear fade in/out avoids audible clicks at beep edges.
                let envelope = if i < fade_len {
                    i as f64 / fade_len as f64
                } else if i + fade_len > beep_len {
                    (beep_len - i) as f64 / fade_len as f64
                } else {
                    1.0
                };
                let t = i as f64 / SAMPLE_RATE as f64;
                let v = (2.0 * std::f64::consts::PI * TONE_HZ * t).sin() * AMPLITUDE * envelope;
                samples.push((v * i16::MAX as f64) as i16);
            }
            if beep + 1 < BEEPS_PER_BURST {
                samples.extend(std::iter::repeat_n(0i16, gap_len));
            }
        }
        if burst + 1 < BURSTS {
            samples.extend(std::iter::repeat_n(0i16, burst_gap_len));
        }
    }
    samples
}

/// Encodes mono 16-bit PCM samples as a RIFF/WAVE byte buffer.
pub fn encode_wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Writes the default alarm WAV to `path` unless an identical file already exists.
pub fn ensure_alert_sound_at(path: &Path) -> io::Result<()> {
    let wav = encode_wav(&generate_alert_samples());
    if let Ok(existing) = fs::read(path) {
        if existing == wav {
            return Ok(());
        }
    }
    crate::paths::write_file_atomic_0600(path, &wav).map_err(io::Error::other)
}

/// Returns the path of the built-in alarm sound, generating it on first use.
pub fn default_alert_sound() -> Option<PathBuf> {
    let path = default_alert_sound_path();
    ensure_alert_sound_at(&path).ok().map(|_| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid_pcm_mono_16bit() {
        let samples = generate_alert_samples();
        assert!(!samples.is_empty());
        let wav = encode_wav(&samples);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1, "PCM");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1, "mono");
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            SAMPLE_RATE
        );
        let data_len = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as usize;
        assert_eq!(data_len, samples.len() * 2);
        assert_eq!(wav.len(), 44 + data_len);
    }

    #[test]
    fn alert_is_audible_and_reasonably_short() {
        let samples = generate_alert_samples();
        let secs = samples.len() as f64 / SAMPLE_RATE as f64;
        assert!((1.0..3.0).contains(&secs), "duration {secs}s");
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > (i16::MAX as u16) / 2, "peak {peak}");
    }

    #[test]
    fn ensure_writes_once_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("llu-alert-{}", std::process::id()));
        let path = dir.join("default-alert.wav");
        ensure_alert_sound_at(&path).unwrap();
        let first = fs::read(&path).unwrap();
        ensure_alert_sound_at(&path).unwrap();
        assert_eq!(first, fs::read(&path).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }
}
