use aulo_speech::{EngineChoice, EngineSpec};

/// Bot and chat choices of one engine kind. A chat overrides its bot, and a
/// bot overrides `[voice.stt]` / `[voice.tts]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Overrides<'a> {
    pub bot: Option<&'a EngineChoice>,
    pub chat: Option<&'a EngineChoice>,
}

/// The engines to try, best first, without duplicates.
///
/// - Every layer that names an engine adds it, chat first, then bot, then
///   config. The configured default stays in the list behind an override so
///   that a broken override falls back to what the user set up, not straight
///   to the fallback list.
/// - A layer that sets only `model` or `voice` refines the next engine below
///   it; a model or voice never carries over to a different engine, where it
///   would most likely be unknown.
/// - `rate` and `fallback` come from the highest layer that sets them.
///   Fallback engines start with their default model and voice.
pub fn candidates(config: &EngineChoice, overrides: Overrides<'_>) -> Vec<EngineSpec> {
    let layers: Vec<&EngineChoice> = [overrides.chat, overrides.bot, Some(config)]
        .into_iter()
        .flatten()
        .collect();
    let rate = layers.iter().find_map(|l| l.rate).unwrap_or_default();
    let fallback = layers
        .iter()
        .find(|l| !l.fallback.is_empty())
        .map_or(&config.fallback, |l| &l.fallback);

    let mut out: Vec<EngineSpec> = Vec::new();
    let (mut model, mut voice) = (None, None);
    for layer in &layers {
        model = model.or(layer.model.as_ref());
        voice = voice.or(layer.voice.as_ref());
        if let Some(engine) = &layer.engine {
            push_new(
                &mut out,
                EngineSpec {
                    engine: engine.clone(),
                    model: model.take().cloned(),
                    voice: voice.take().cloned(),
                    rate,
                },
            );
        }
    }
    for engine in fallback {
        push_new(
            &mut out,
            EngineSpec {
                engine: engine.clone(),
                model: None,
                voice: None,
                rate,
            },
        );
    }
    out
}

/// The first mention of an engine wins: it carries the more specific model
/// and voice, and retrying a failed engine later in the same list is useless.
fn push_new(out: &mut Vec<EngineSpec>, spec: EngineSpec) {
    if !out.iter().any(|s| s.engine == spec.engine) {
        out.push(spec);
    }
}
