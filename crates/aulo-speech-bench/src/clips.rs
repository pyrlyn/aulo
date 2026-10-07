//! The workload: the fixture clips of the sherpa and system engine tests
//! (macOS `say`, Samantha and Milena, 16 kHz mono 16-bit), embedded so the
//! installed binary needs no files next to it. Speech-to-text hears all five;
//! text-to-speech reads the three English sentences.

use std::io::Cursor;
use std::time::Duration;

use aulo_speech::PIPELINE_SAMPLE_RATE_HZ;

/// One recorded sentence and what was said in it.
#[derive(Debug, Clone, Copy)]
pub struct Clip {
    pub name: &'static str,
    /// BCP 47 primary tag, passed to the engine as the language hint.
    pub language: &'static str,
    pub text: &'static str,
    wav: &'static [u8],
}

macro_rules! clip {
    ($name:literal, $language:literal, $text:literal) => {
        Clip {
            name: $name,
            language: $language,
            text: $text,
            wav: include_bytes!(concat!(
                "../../aulo-speech-sherpa/tests/fixtures/",
                $name,
                ".wav"
            )),
        }
    };
}

pub const CLIPS: [Clip; 5] = [
    clip!("en_browser", "en", "Open the browser and play some music."),
    clip!(
        "en_weather",
        "en",
        "What is the weather like in London tomorrow?"
    ),
    clip!("en_email", "en", "Please read my latest email out loud."),
    clip!("ru_browser", "ru", "Открой браузер и включи музыку."),
    clip!("ru_weather", "ru", "Какая завтра погода в Москве?"),
];

impl Clip {
    /// Decodes the clip to pipeline-format samples.
    pub fn samples(&self) -> Result<Vec<f32>, String> {
        let mut reader = hound::WavReader::new(Cursor::new(self.wav))
            .map_err(|e| format!("{}: {e}", self.name))?;
        let spec = reader.spec();
        if (spec.sample_rate, spec.channels, spec.bits_per_sample)
            != (PIPELINE_SAMPLE_RATE_HZ, 1, 16)
        {
            return Err(format!("{}: not 16 kHz mono 16-bit", self.name));
        }
        reader
            .samples::<i16>()
            .map(|s| {
                s.map(|v| f32::from(v) / f32::from(i16::MAX))
                    .map_err(|e| format!("{}: {e}", self.name))
            })
            .collect()
    }
}

pub fn duration_of(samples: &[f32]) -> Duration {
    Duration::from_secs_f64(samples.len() as f64 / f64::from(PIPELINE_SAMPLE_RATE_HZ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clip_decodes_to_a_few_seconds_of_audio() {
        for clip in CLIPS {
            let seconds = duration_of(&clip.samples().unwrap()).as_secs_f64();
            assert!((1.0..10.0).contains(&seconds), "{}: {seconds} s", clip.name);
        }
    }
}
