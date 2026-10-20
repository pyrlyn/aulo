//! Windows named-pipe client identity.
//!
//! `GetNamedPipeClientProcessId` and the token APIs are raw Win32, so this
//! module is the only place `aulo-server` allows `unsafe`. A failed read
//! becomes `PipeClient::Unknown`, which the interceptor refuses.

#![allow(unsafe_code)]

use std::os::windows::io::AsRawHandle;

use tokio::net::windows::named_pipe::NamedPipeServer;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{
    CopySid, GetLengthSid, GetTokenInformation, IsValidSid, PSID, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::Owned;

use super::{AccountSid, PipeClient, SID_CAP, classify_pipe_client};

/// Bytes asked of `GetTokenInformation` for `TokenUser`, SID included.
const TOKEN_USER_BYTES: usize = 256;

/// Eight-byte alignment so a `TOKEN_USER` (it holds a pointer) can be read
/// back out of the buffer Windows fills.
#[repr(C, align(8))]
struct AlignedBytes<const N: usize> {
    bytes: [u8; N],
}

pub(super) fn probe(server: &NamedPipeServer) -> PipeClient {
    let Some(pid) = client_pid(server) else {
        return PipeClient::Unknown;
    };
    let client = account_of_pid(pid);
    let owner = current_account();
    // Read the pid again after the token reads. A process that exited in
    // between can have its id reused; if the pipe no longer names `pid`,
    // the account we just read belongs to someone else.
    let pid_unchanged = client_pid(server) == Some(pid);
    classify_pipe_client(client.as_ref(), owner.as_ref(), pid_unchanged)
}

fn client_pid(server: &NamedPipeServer) -> Option<u32> {
    let pipe = HANDLE(server.as_raw_handle());
    let mut pid = 0u32;
    // SAFETY: `pipe` is the server end, and the caller probes only after
    // `connect` has succeeded. `pid` is a local out-parameter.
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) }.is_err() {
        return None;
    }
    (pid != 0).then_some(pid)
}

fn current_account() -> Option<AccountSid> {
    // SAFETY: the pseudo-handle is only valid in this process and must not
    // be closed. `account_of` does not close the process handle it is given.
    let process = unsafe { GetCurrentProcess() };
    account_of(process)
}

fn account_of_pid(pid: u32) -> Option<AccountSid> {
    if pid == 0 {
        return None;
    }
    // SAFETY: a process id, no pointers. Failure opens nothing.
    let opened = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let process = opened.ok()?;
    // SAFETY: `OpenProcess` returned an owned handle. It is not the
    // `GetCurrentProcess` pseudo-handle, so closing it on drop is required.
    let process = unsafe { Owned::new(process) };
    account_of(*process)
}

fn account_of(process: HANDLE) -> Option<AccountSid> {
    let mut raw = HANDLE::default();
    // SAFETY: `process` is a live process handle. On success `raw` is an
    // owned token; on failure it is left invalid and must not be closed.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) }.is_err() {
        return None;
    }
    if raw.is_invalid() {
        return None;
    }
    // SAFETY: `OpenProcessToken` succeeded, so `raw` is an owned token.
    let token = unsafe { Owned::new(raw) };
    token_user(*token)
}

fn token_user(token: HANDLE) -> Option<AccountSid> {
    let mut needed = 0u32;
    // SAFETY: a null buffer of length 0 asks for the size. The error is
    // expected (`ERROR_INSUFFICIENT_BUFFER`); `needed` is a local out-parameter.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut needed) };
    let needed_usize = usize::try_from(needed).ok()?;
    if needed_usize == 0 || needed_usize > TOKEN_USER_BYTES {
        return None;
    }
    let mut buf = AlignedBytes::<TOKEN_USER_BYTES> {
        bytes: [0; TOKEN_USER_BYTES],
    };
    // SAFETY: `buf` is 8-byte aligned and longer than `needed`. On success
    // Windows writes a `TOKEN_USER` whose `Sid` points into `buf`.
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.bytes.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
    }
    .ok()?;
    // SAFETY: the call succeeded, so `buf` starts with a `TOKEN_USER`. The
    // SID it points at stays valid until `buf` drops, which is after the copy.
    let sid = unsafe { (*buf.bytes.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    copy_sid(sid)
}

fn copy_sid(source: PSID) -> Option<AccountSid> {
    if source.is_invalid() {
        return None;
    }
    // SAFETY: `source` is the SID from the `TOKEN_USER` still held by the caller.
    if !unsafe { IsValidSid(source) }.as_bool() {
        return None;
    }
    // SAFETY: `IsValidSid` just accepted `source`.
    let len = unsafe { GetLengthSid(source) };
    let len_usize = usize::try_from(len).ok()?;
    if len_usize == 0 || len_usize > SID_CAP {
        return None;
    }
    let mut copied = AlignedBytes::<SID_CAP> {
        bytes: [0; SID_CAP],
    };
    let dest = PSID(copied.bytes.as_mut_ptr().cast());
    // SAFETY: `dest` has room for `len` bytes (`len_usize <= SID_CAP`) and is
    // aligned. `source` is the valid SID checked above and still alive.
    unsafe { CopySid(len, dest, source) }.ok()?;
    AccountSid::from_bytes(&copied.bytes[..len_usize])
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    use super::probe;
    use crate::auth::PipeClient;

    fn pipe_name() -> String {
        static N: AtomicU64 = AtomicU64::new(0);
        format!(
            r"\\.\pipe\aulo-pipe-owner-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[tokio::test]
    async fn the_connected_client_is_the_daemon_user() {
        let name = pipe_name();
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&name)
            .unwrap();
        let opened = tokio::task::spawn_blocking(move || ClientOptions::new().open(name));
        server.connect().await.unwrap();
        assert_eq!(probe(&server), PipeClient::Owner);
        opened.await.unwrap().unwrap();
    }
}
