use super::audio::{self, Target};
/// Presentation retains only the voices that can still contribute samples.
/// Their identities, timestamps and PCM all come from the existing JS core.
pub(super) struct AudioCursor {
    pub(super) target: Target,
    pub(super) sample: usize,
    pub(super) voices: Vec<serde_json::Value>,
}

impl AudioCursor {
    pub(super) fn new(target: Target, time_ms: f64) -> Self {
        Self {
            target,
            sample: (time_ms * audio::SAMPLE_RATE as f64 / 1000.0).floor() as usize,
            voices: Vec::new(),
        }
    }
    pub(super) fn present(
        &mut self,
        ctx: &rquickjs::Ctx<'_>,
        target: &Target,
        time_ms: f64,
    ) -> rquickjs::Result<()> {
        if !self.target.same_stream(target) {
            self.target = target.clone();
            self.sample = (time_ms * audio::SAMPLE_RATE as f64 / 1000.0).floor() as usize;
            self.voices.clear();
        }
        if !target.current() {
            self.sample = (time_ms * audio::SAMPLE_RATE as f64 / 1000.0).floor() as usize;
            self.voices.clear();
            return Ok(());
        }
        if let Some(channels) = self.render_samples(ctx, time_ms)? {
            target
                .send(channels)
                .map_err(|_| rquickjs::Error::Unknown)?;
        }
        Ok(())
    }

    pub(super) fn render_samples(
        &mut self,
        ctx: &rquickjs::Ctx<'_>,
        time_ms: f64,
    ) -> rquickjs::Result<Option<[Vec<f32>; 2]>> {
        let end = (time_ms * audio::SAMPLE_RATE as f64 / 1000.0).floor() as usize;
        // A pause, new output or delayed catch-up cannot play historical sound.
        if end < self.sample || end - self.sample > audio::MAX_FRAMES {
            self.sample = end;
            self.voices.clear();
        }
        let json: String = ctx.eval("JSON.stringify(JSON.parse(pet.snapshot()).voices)")?;
        let voices: Vec<serde_json::Value> =
            serde_json::from_str(&json).map_err(|_| rquickjs::Error::Unknown)?;
        let start = self.sample as f64 / audio::SAMPLE_RATE as f64;
        self.voices.extend(voices);
        self.voices.retain(|v| {
            v["start"]
                .as_f64()
                .zip(v["duration"].as_f64())
                .is_some_and(|(at, duration)| at + duration > start)
        });
        if self.voices.len() > 128 {
            return Err(rquickjs::Error::Unknown);
        }
        if end == self.sample {
            return Ok(None);
        }
        ctx.globals().set(
            "petAudioVoices",
            serde_json::to_string(&self.voices).map_err(|_| rquickjs::Error::Unknown)?,
        )?;
        ctx.globals().set("petAudioStart", self.sample)?;
        ctx.globals().set("petAudioLength", end - self.sample)?;
        let json: String =
            ctx.eval("pet.pcm(petAudioVoices, petAudioStart, petAudioLength, 48000)")?;
        let channels: [Vec<f32>; 2] =
            serde_json::from_str(&json).map_err(|_| rquickjs::Error::Unknown)?;
        if channels[0].len() != end - self.sample || channels[1].len() != end - self.sample {
            return Err(rquickjs::Error::Unknown);
        }
        self.sample = end;
        Ok(Some(channels))
    }
}
