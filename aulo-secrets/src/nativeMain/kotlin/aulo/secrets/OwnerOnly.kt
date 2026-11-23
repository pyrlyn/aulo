package aulo.secrets

/** A path cannot be made, or is not, reachable by its owner only. */
class OwnerOnlyException(
    message: String,
) : Exception(message)

/**
 * Makes [dir] reachable by the current user only, creating it (not its parents) if needed.
 * An existing directory passes only if the user owns it and it has no group or other bits:
 * a looser one may already be some other process's working directory, from where everything
 * created inside stays reachable. A symlink or a file at [dir] is refused.
 */
expect fun ensureOwnerOnlyDir(dir: String)

/** Sets [file] to 0600. */
expect fun restrictToOwner(file: String)
