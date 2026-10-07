//! Size caps for manifest input. They bound the work a hostile file can cause
//! and are mirrored in the JSON Schema.

/// Real manifests are well under 4 KiB; the margin is for long host lists.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// `plugin__<id>__<tool>` must still fit a provider's 64-character tool name.
pub const MAX_ID_LEN: usize = 32;
pub const MAX_HOSTS: usize = 64;
pub const MAX_FS_ROOTS: usize = 64;
pub const MAX_ARGS: usize = 64;
/// Longest single string (argument, path); matches common `PATH_MAX`.
pub const MAX_STRING_LEN: usize = 4096;
/// RFC 1035 limit for a full DNS name.
pub const MAX_HOST_LEN: usize = 253;
