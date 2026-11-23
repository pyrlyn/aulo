package aulo.telemetry

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Severity, least severe first, so `at >= threshold` means "log it". */
@Serializable
enum class LogLevel {
    @SerialName("trace")
    TRACE,

    @SerialName("debug")
    DEBUG,

    @SerialName("info")
    INFO,

    @SerialName("warn")
    WARN,

    @SerialName("error")
    ERROR,

    ;

    val label: String get() = name.lowercase()
}
