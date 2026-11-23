package aulo.voice

import aulo.speech.EngineChoice
import aulo.speech.EngineId
import aulo.speech.EngineSpec
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.testkit.FAKE_VOICE
import aulo.testkit.FRAME_SAMPLES
import aulo.testkit.FakeStt
import aulo.testkit.FakeTts
import aulo.types.AuloEvent
import aulo.types.NoticeLevel
import aulo.types.TurnId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Registry, selection, fallback and switching, driven with aulo-testkit's fake engines.

private fun id(id: String) = EngineId(id)

private fun choice(
    engine: String,
    vararg fallback: String,
) = EngineChoice(engine = id(engine), fallback = fallback.map(::id))

private fun List<EngineSpec>.ids() = map { it.engine.value }

/** `alpha` and `beta` work, `broken` builds and then reports itself unavailable, `missing` cannot be built. */
private fun ttsRegistry() =
    TtsRegistry().apply {
        listOf("alpha", "beta").forEach { name -> register(id(name)) { FakeTts(name) } }
        register(id("broken")) { FakeTts("broken", SpeechException.Unavailable("plugin process exited")) }
        register(id("missing")) { throw SpeechException.Unavailable("model file not found") }
    }

private class Notices : (AuloEvent) -> Unit {
    val all = mutableListOf<AuloEvent.Notice>()

    override fun invoke(event: AuloEvent) {
        all += event as AuloEvent.Notice
    }

    fun sources(): List<String?> =
        all.map {
            assertEquals(NoticeLevel.WARN, it.level)
            it.source
        }
}

private fun ActiveTts.speak(
    registry: TtsRegistry,
    notices: Notices,
): String {
    begin(registry, notices, TurnId.new(), "en")
    return checkNotNull(active).value
}

class RegistryTest {
    @Test
    fun chat_overrides_bot_which_overrides_config() {
        val config = choice("alpha", "gamma").copy(voice = "config-voice", rate = SpeechRate(1.2f))
        val bot = choice("beta").copy(model = "bot-model")
        val specs = candidates(config, bot = bot, chat = choice("delta"))
        assertEquals(listOf("delta", "beta", "alpha", "gamma"), specs.ids())
        // Model and voice stay with the layer that chose the engine.
        assertNull(specs[0].model)
        assertEquals("bot-model", specs[1].model)
        assertEquals("config-voice", specs[2].voice)
        assertNull(specs[3].voice)
        // Rate is not tied to an engine, so the configured one applies to all.
        assertTrue(specs.all { it.rate == SpeechRate(1.2f) })
    }

    @Test
    fun a_voice_only_override_refines_the_engine_below_it() {
        val config = choice("alpha", "alpha", "beta").copy(voice = "config-voice")
        val chat = EngineChoice(voice = "chat-voice", rate = SpeechRate(0.8f))
        val specs = candidates(config, chat = chat)
        assertEquals(listOf("alpha", "beta"), specs.ids())
        assertEquals("chat-voice", specs[0].voice)
        assertEquals(SpeechRate(0.8f), specs[1].rate)
    }

    @Test
    fun the_highest_fallback_list_wins() {
        val specs = candidates(choice("alpha", "beta"), bot = EngineChoice(fallback = listOf(id("gamma"))))
        assertEquals(listOf("alpha", "gamma"), specs.ids())
    }

    @Test
    fun nothing_configured_means_no_engine() {
        val tts = ActiveTts(candidates(EngineChoice()))
        val notices = Notices()
        val error = assertFailsWith<RegistryException.NoEngine> { tts.begin(ttsRegistry(), notices, TurnId.new(), null) }
        assertEquals("no tts engine could start", error.message)
        assertTrue(notices.all.isEmpty())
    }

    @Test
    fun switches_tts_mid_chat() {
        val registry = ttsRegistry()
        val config = choice("alpha")
        val tts = ActiveTts(candidates(config))
        val notices = Notices()
        val out = FloatArray(FRAME_SAMPLES)

        assertEquals("alpha", tts.speak(registry, notices))
        val alpha = checkNotNull(tts.engine) as FakeTts
        alpha.pushText("First sentence.")
        assertTrue(alpha.poll(out).samples > 0)

        // `aulo voice use beta` arrives while the reply is being spoken.
        tts.switch(candidates(config, chat = choice("beta")))
        assertEquals("alpha", tts.active?.value)
        alpha.pushText("Second sentence.")
        alpha.finish()
        while (!alpha.poll(out).isDone) {}

        assertEquals("beta", tts.speak(registry, notices))
        assertTrue(alpha.closed)
        assertTrue(notices.all.isEmpty())
    }

    @Test
    fun a_failing_engine_falls_back_with_a_notice() {
        val registry = ttsRegistry()
        val tts = ActiveTts(candidates(choice("broken", "missing", "unknown", "beta")))
        val notices = Notices()

        assertEquals("beta", tts.speak(registry, notices))
        assertEquals(listOf("broken", "missing", "unknown"), notices.sources())
        assertEquals(
            "tts engine `broken` failed, trying the next one: engine unavailable: plugin process exited",
            notices.all[0].message,
        )
        assertTrue(notices.all[2].message.endsWith("engine is not registered"), notices.all[2].message)

        // The working engine stays in use; the broken ones are not retried.
        notices.all.clear()
        assertEquals("beta", tts.speak(registry, notices))
        assertTrue(notices.all.isEmpty())
    }

    @Test
    fun a_voice_the_engine_lacks_falls_back_to_the_next_engine() {
        val registry = ttsRegistry()
        val tts = ActiveTts(candidates(choice("alpha", "beta").copy(voice = "boris")))
        val notices = Notices()
        assertEquals("beta", tts.speak(registry, notices))
        assertEquals(listOf("alpha"), notices.sources())

        tts.switch(candidates(choice("alpha").copy(voice = FAKE_VOICE)))
        assertEquals("alpha", tts.speak(registry, notices))
    }

    @Test
    fun an_engine_failing_mid_reply_is_replaced_from_the_next_reply() {
        val registry = ttsRegistry()
        val tts = ActiveTts(candidates(choice("alpha", "beta")))
        val notices = Notices()
        assertEquals("alpha", tts.speak(registry, notices))
        val alpha = checkNotNull(tts.engine) as FakeTts

        // A caller bug keeps the engine.
        tts.report(SpeechException.OutOfOrder("poll before begin"), notices)
        assertEquals("alpha", tts.active?.value)
        // An engine fault retires and closes it, without starting another mid-reply.
        tts.report(SpeechException.Failed("socket closed"), notices)
        assertNull(tts.active)
        assertTrue(alpha.closed)

        assertEquals("beta", tts.speak(registry, notices))
        assertEquals(listOf("alpha"), notices.sources())
    }

    @Test
    fun caller_bugs_do_not_trigger_fallback() {
        val registry =
            SttRegistry().apply {
                register(id("strict")) { FakeStt("strict", SpeechException.OutOfOrder("begin while closed")) }
                register(id("spare")) { FakeStt("spare") }
            }
        val stt = ActiveStt(candidates(choice("strict", "spare")))
        val notices = Notices()
        val error = assertFailsWith<SpeechException.OutOfOrder> { stt.begin(registry, notices, TurnId.new(), null) }
        assertEquals("called out of order: begin while closed", error.message)
        assertEquals("strict", stt.active?.value)
        assertTrue(notices.all.isEmpty())
    }

    @Test
    fun when_every_engine_fails_the_next_begin_starts_over() {
        val registry = ttsRegistry()
        val tts = ActiveTts(candidates(choice("broken", "missing")))
        repeat(2) {
            val notices = Notices()
            assertFailsWith<RegistryException.NoEngine> { tts.begin(registry, notices, TurnId.new(), null) }
            assertEquals(listOf("broken", "missing"), notices.sources())
        }
    }

    @Test
    fun a_plugin_cannot_take_over_a_registered_id() {
        val registry = ttsRegistry()
        val error = assertFailsWith<RegistryException.Duplicate> { registry.register(id("alpha")) { FakeTts("impostor") } }
        assertEquals("tts engine `alpha` is already registered", error.message)
        assertEquals(listOf("alpha", "beta", "broken", "missing"), registry.ids().map { it.value })
        assertTrue(id("beta") in registry)
    }

    @Test
    fun the_factory_receives_the_resolved_spec() {
        var seen: EngineSpec? = null
        val registry = TtsRegistry().apply { register(id("echo")) { spec -> FakeTts(spec.engine.value).also { seen = spec } } }
        val config = choice("echo").copy(model = "kokoro-v1", rate = SpeechRate(1.5f))
        assertEquals("echo", ActiveTts(candidates(config)).speak(registry, Notices()))
        assertEquals(EngineSpec(id("echo"), model = "kokoro-v1", rate = SpeechRate(1.5f)), seen)
    }

    @Test
    fun an_engine_that_breaks_the_contract_still_falls_back() {
        val registry =
            TtsRegistry().apply {
                register(id("rogue")) { error("factory crashed") }
                register(id("alpha")) { FakeTts("alpha") }
            }
        val tts = ActiveTts(candidates(choice("rogue", "alpha")))
        val notices = Notices()
        assertEquals("alpha", tts.speak(registry, notices))
        assertEquals(
            "tts engine `rogue` failed, trying the next one: engine failed: IllegalStateException: factory crashed",
            notices.all.single().message,
        )
    }

    @Test
    fun a_long_engine_error_is_capped_in_the_notice() {
        val flood = "x".repeat(AuloEvent.Notice.MAX_MESSAGE_BYTES * 4)
        val registry = TtsRegistry().apply { register(id("noisy")) { throw SpeechException.Failed(flood) } }
        val notices = Notices()
        assertFailsWith<RegistryException.NoEngine> { ActiveTts(candidates(choice("noisy"))).begin(registry, notices, TurnId.new(), null) }
        assertTrue(
            notices.all
                .single()
                .message
                .encodeToByteArray()
                .size <= AuloEvent.Notice.MAX_MESSAGE_BYTES,
        )
    }
}
