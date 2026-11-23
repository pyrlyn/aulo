package aulo.models

/** Why a model could not be listed, installed or removed. Messages name files and URLs from the catalog only. */
sealed class ModelException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    class BadCatalog(
        detail: String,
    ) : ModelException("invalid model catalog: $detail")

    class UnknownModel(
        val id: String,
    ) : ModelException("unknown model `$id`")

    /** A name that is not a plain directory name, so it never reaches a path. */
    class UnsafeName : ModelException("not a model id: only a-z, 0-9, '.', '_' and '-', starting with a letter or digit")

    class Http(
        val url: String,
        val status: Int,
    ) : ModelException("$url answered HTTP $status")

    class BadRange(
        val url: String,
    ) : ModelException("$url ignored the requested range or answered with the wrong one")

    class Network(
        val url: String,
        cause: Throwable,
    ) : ModelException("download of $url failed: ${cause.message}", cause)

    class HashMismatch(
        val file: String,
        val expected: String,
        val actual: String,
    ) : ModelException("$file: SHA-256 is $actual, the catalog pins $expected; nothing was kept")

    class TooLarge(
        val file: String,
        val limit: Long,
    ) : ModelException("$file: the server sent more than the pinned $limit bytes; nothing was kept")

    class Truncated(
        val file: String,
        val got: Long,
        val expected: Long,
    ) : ModelException("$file: the download stopped at $got of $expected bytes; run the pull again to resume")

    /** Another pull or removal of the same model holds its lock. */
    class Busy(
        val id: String,
    ) : ModelException("model `$id` is being downloaded or removed by another aulo; try again when it finishes")

    class Io(
        val path: String,
        cause: Throwable,
    ) : ModelException("$path: ${ioDetail(cause)}", cause)
}
