package aulo.config

/** Where a config value came from, lowest layer first. */
sealed interface Origin {
    data object Default : Origin {
        override fun toString() = "built-in defaults"
    }

    data class File(
        val path: String,
    ) : Origin {
        override fun toString() = path
    }

    /** The variable's name, for example `AULO_VOICE__LISTEN`. */
    data class Env(
        val name: String,
    ) : Origin {
        override fun toString() = name
    }

    data object Override : Origin {
        override fun toString() = "command-line overrides"
    }
}

/**
 * A layer that cannot be used. The message names the layer and the dotted key but never the
 * value: a misplaced secret must not end up in a terminal or a log.
 */
class ConfigException(
    val origin: Origin?,
    val key: String?,
    val detail: String,
) : Exception(if (origin == null) detail else "$origin: $detail")
