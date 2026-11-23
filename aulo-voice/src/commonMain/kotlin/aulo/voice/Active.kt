package aulo.voice

import aulo.speech.AudioFormat
import aulo.speech.EngineId
import aulo.speech.EngineKind
import aulo.speech.EngineSpec
import aulo.speech.SpeechEngine
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.TtsEngine
import aulo.speech.TtsRequest
import aulo.types.AuloEvent
import aulo.types.NoticeLevel
import aulo.types.TurnId

/** Where the engine in use reports engines it skipped; clients show these as warnings. */
typealias Notify = (AuloEvent) -> Unit

/**
 * The engine one bot or chat listens or speaks with, and the ordered candidates behind it.
 *
 * Engines are built lazily, when an utterance or reply begins, and stay in use while they work.
 * A failing engine is closed and the next candidate tried; once every candidate has failed, the
 * next begin starts over from the first, so an engine that recovers (network back, plugin
 * restarted) is picked up again.
 */
sealed class ActiveEngine<E : SpeechEngine>(
    val kind: EngineKind,
    candidates: List<EngineSpec>,
) {
    private var candidates = candidates.toList()

    /** Index of the candidate to build when there is no engine. */
    private var next = 0
    private var spec: EngineSpec? = null
    private var current: E? = null

    /** A switch waiting for the next begin, so an utterance or reply finishes on the engine it started with. */
    private var pending: List<EngineSpec>? = null

    /**
     * Replaces the candidates from the next begin on. `VoiceService.SetVoice`, `aulo voice use`
     * and voice commands call it at any time, including mid-utterance.
     */
    fun switch(candidates: List<EngineSpec>) {
        pending = candidates.toList()
    }

    /** The engine in use, for pushing and polling after a successful begin. */
    val engine: E? get() = current

    /** The registry id of the engine in use, from the spec: a plugin's own info says whatever it likes. */
    val active: EngineId? get() = spec?.engine

    /**
     * Reports an error the engine threw mid-utterance. An engine fault retires the engine, so the
     * next begin moves on to the next candidate; the utterance itself is not moved to another
     * engine halfway through. A caller bug leaves the engine in place.
     */
    fun report(
        error: SpeechException,
        notify: Notify,
    ) {
        if (!error.shouldFallBack) return
        val spec = spec ?: return
        notify(notice("${kind.label} engine `${spec.engine.value}` failed, trying the next one: ${error.message}", spec))
        drop(notify)
    }

    /** Closes the engine in use; the next begin builds one again. */
    fun close(notify: Notify) {
        drop(notify)
    }

    /** Applies a pending switch, then starts the first candidate that builds and accepts [start]. */
    protected fun <T> beginWith(
        registry: EngineRegistry<E>,
        notify: Notify,
        start: (E, EngineSpec) -> T,
    ): T {
        pending?.let {
            drop(notify)
            candidates = it
            next = 0
            pending = null
        }
        // Every pass either returns or uses up one candidate, so this ends.
        while (true) {
            val engine = current ?: build(registry, notify) ?: continue
            val spec = checkNotNull(spec)
            try {
                return guarded { start(engine, spec) }
            } catch (e: SpeechException) {
                if (!e.shouldFallBack) throw e
                report(e, notify)
            }
        }
    }

    /** Builds the next candidate; null when it failed and was noted. Building is the engine's own business, so every build error falls back. */
    private fun build(
        registry: EngineRegistry<E>,
        notify: Notify,
    ): E? {
        val candidate = candidates.getOrNull(next)
        if (candidate == null) {
            next = 0
            throw RegistryException.NoEngine(kind)
        }
        next++
        return try {
            registry.build(candidate).also {
                spec = candidate
                current = it
            }
        } catch (e: SpeechException) {
            notify(notice("${kind.label} engine `${candidate.engine.value}` failed, trying the next one: ${e.message}", candidate))
            null
        }
    }

    private fun drop(notify: Notify) {
        val engine = current ?: return
        val spec = checkNotNull(spec)
        current = null
        this.spec = null
        try {
            guarded { engine.close() }
        } catch (e: SpeechException) {
            notify(notice("${kind.label} engine `${spec.engine.value}` did not close cleanly: ${e.message}", spec))
        }
    }

    // The id is bounded by its alphabet and length and the message by SpeechException's detail
    // cap, and capped() bounds both again, so a plugin cannot flood clients here.
    private fun notice(
        message: String,
        spec: EngineSpec,
    ) = AuloEvent.Notice.capped(NoticeLevel.WARN, message, source = spec.engine.value)
}

class ActiveStt(
    candidates: List<EngineSpec>,
) : ActiveEngine<SttEngine>(EngineKind.STT, candidates) {
    /** Starts an utterance; see [SttEngine.begin]. */
    fun begin(
        registry: SttRegistry,
        notify: Notify,
        turnId: TurnId,
        language: String?,
    ) = beginWith(registry, notify) { engine, _ -> engine.begin(turnId, language) }
}

class ActiveTts(
    candidates: List<EngineSpec>,
) : ActiveEngine<TtsEngine>(EngineKind.TTS, candidates) {
    /** Starts a reply with the candidate's voice and rate; see [TtsEngine.begin]. */
    fun begin(
        registry: TtsRegistry,
        notify: Notify,
        turnId: TurnId,
        language: String?,
    ): AudioFormat =
        beginWith(registry, notify) { engine, spec ->
            engine.begin(TtsRequest(turnId, voice = spec.voice, language = language, rate = spec.rate))
        }
}
