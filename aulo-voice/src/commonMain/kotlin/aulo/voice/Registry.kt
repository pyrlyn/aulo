package aulo.voice

import aulo.speech.EngineId
import aulo.speech.EngineKind
import aulo.speech.EngineSpec
import aulo.speech.SpeechEngine
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.TtsEngine

/** Builds one engine from a resolved spec. A plugin's factory talks to its plugin process. */
typealias EngineFactory<E> = (EngineSpec) -> E

/** What the registry and the engine in use refuse; engine faults stay [SpeechException]s. */
sealed class RegistryException(
    message: String,
) : Exception(message) {
    /** A second engine, most likely a plugin, tried to take an id that is already registered. */
    class Duplicate(
        val kind: EngineKind,
        val id: EngineId,
    ) : RegistryException("${kind.label} engine `${id.value}` is already registered")

    /** Every candidate failed to build or start, or none was configured. */
    class NoEngine(
        val kind: EngineKind,
    ) : RegistryException("no ${kind.label} engine could start")
}

internal val EngineKind.label: String get() = name.lowercase()

/**
 * Factories for one engine kind, by id. Built-in engines, cloud clients and plugins register
 * here at startup; nothing is built until an utterance or reply needs it.
 */
sealed class EngineRegistry<E : SpeechEngine>(
    val kind: EngineKind,
) {
    private val factories = mutableMapOf<EngineId, EngineFactory<E>>()

    fun register(
        id: EngineId,
        factory: EngineFactory<E>,
    ) {
        if (id in factories) throw RegistryException.Duplicate(kind, id)
        factories[id] = factory
    }

    /** Registered ids in a stable order, for pickers and `aulo voice list`. */
    fun ids(): List<EngineId> = factories.keys.sortedBy { it.value }

    operator fun contains(id: EngineId): Boolean = id in factories

    /**
     * An id nobody registered is reported as unavailable rather than as a caller error: a config
     * naming an uninstalled plugin must fall back. A factory is extension code, so anything it
     * throws becomes a fallback-able [SpeechException].
     */
    fun build(spec: EngineSpec): E {
        val factory = factories[spec.engine] ?: throw SpeechException.Unavailable("engine is not registered")
        return guarded { factory(spec) }
    }
}

class SttRegistry : EngineRegistry<SttEngine>(EngineKind.STT)

class TtsRegistry : EngineRegistry<TtsEngine>(EngineKind.TTS)

/**
 * Runs extension code. The contract lets engines throw only [SpeechException]; anything else is
 * a broken engine, so it is treated as an engine fault and the next engine is tried.
 */
internal inline fun <T> guarded(block: () -> T): T =
    try {
        block()
    } catch (e: SpeechException) {
        throw e
    } catch (e: Exception) {
        throw SpeechException.Failed(listOfNotNull(e::class.simpleName, e.message).joinToString(": "))
    }
