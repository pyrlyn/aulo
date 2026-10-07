use std::ffi::OsString;
use std::path::{Path, PathBuf};

use reqwest::{Client, Response, StatusCode, header};
use sha2::{Digest, Sha256};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::catalog::PART_SUFFIX;
use crate::error::ModelError;

/// What one file must look like when the download is done.
pub(crate) struct Expected<'a> {
    pub url: &'a str,
    /// Name used in errors.
    pub name: &'a str,
    pub size: u64,
    pub sha256: &'a str,
}

/// Downloads `want.url` to `dest` and returns only once the bytes match the
/// pinned SHA-256.
///
/// The body streams into `<dest>.part` and is hashed on the way. A hash, size
/// or HTTP failure deletes the partial, so the final name never holds bytes
/// that were not checked. A dropped connection or a cancelled future keeps
/// the partial, and the next call resumes it with a `Range` request.
/// `on_bytes` receives the bytes this file has so far.
pub(crate) async fn fetch_verified(
    client: &Client,
    want: &Expected<'_>,
    dest: &Path,
    mut on_bytes: impl FnMut(u64),
) -> Result<(), ModelError> {
    let part = part_path(dest);
    let mut hasher = Sha256::new();
    let mut have = resumable_len(&part, want.size).await;
    if have > 0 {
        hash_file(&part, &mut hasher).await?;
        on_bytes(have);
    }

    if have < want.size {
        let mut request = client.get(want.url);
        if have > 0 {
            request = request.header(header::RANGE, format!("bytes={have}-"));
        }
        let response = request.send().await?;
        let status = response.status();
        if status == StatusCode::OK && have > 0 {
            // The server ignored the range; what we hold is of no use.
            have = 0;
            hasher = Sha256::new();
        } else if status == StatusCode::PARTIAL_CONTENT && have > 0 {
            if range_start(&response) != Some(have) {
                discard(&part).await;
                return Err(ModelError::BadRange {
                    url: want.url.into(),
                });
            }
        } else if status != StatusCode::OK {
            if status == StatusCode::RANGE_NOT_SATISFIABLE {
                discard(&part).await;
            }
            return Err(ModelError::Http {
                url: want.url.into(),
                status: status.as_u16(),
            });
        }
        // Refuse early when the server announces more than the pin allows.
        if response
            .content_length()
            .is_some_and(|len| have + len > want.size)
        {
            discard(&part).await;
            return Err(ModelError::TooLarge {
                file: want.name.into(),
                limit: want.size,
            });
        }
        let result = stream_to(response, &part, have, want, &mut hasher, &mut on_bytes).await;
        if matches!(
            result,
            Err(ModelError::TooLarge { .. } | ModelError::Io { .. })
        ) {
            discard(&part).await;
        }
        have = result?;
    }

    if have != want.size {
        return Err(ModelError::Truncated {
            file: want.name.into(),
            got: have,
            expected: want.size,
        });
    }
    let actual = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if actual != want.sha256 {
        discard(&part).await;
        return Err(ModelError::HashMismatch {
            file: want.name.into(),
            expected: want.sha256.into(),
            actual,
        });
    }
    // The partial was fsynced, so the rename publishes bytes that are on disk.
    fs::rename(&part, dest).await.map_err(ModelError::io(dest))
}

/// Appends the body to the partial (or restarts it when `have` is 0) and
/// returns the new length. The file is flushed even when the stream fails, so
/// a later resume sees every byte that was hashed.
async fn stream_to(
    mut response: Response,
    part: &Path,
    have: u64,
    want: &Expected<'_>,
    hasher: &mut Sha256,
    on_bytes: &mut impl FnMut(u64),
) -> Result<u64, ModelError> {
    let io = ModelError::io(part);
    let mut options = OpenOptions::new();
    if have > 0 {
        options.append(true);
    } else {
        options.write(true).create(true).truncate(true);
    }
    let mut file = options.open(part).await.map_err(&io)?;
    let mut total = have;
    let streamed = loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                total += chunk.len() as u64;
                if total > want.size {
                    break Err(ModelError::TooLarge {
                        file: want.name.into(),
                        limit: want.size,
                    });
                }
                hasher.update(&chunk);
                if let Err(e) = file.write_all(&chunk).await {
                    break Err(io(e));
                }
                on_bytes(total);
            }
            Ok(None) => break Ok(()),
            Err(e) => break Err(e.into()),
        }
    };
    let flushed = file.flush().await;
    streamed?;
    flushed.map_err(&io)?;
    file.sync_all().await.map_err(&io)?;
    Ok(total)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name: OsString = dest.file_name().unwrap_or_default().to_owned();
    name.push(PART_SUFFIX);
    dest.with_file_name(name)
}

/// Length of a usable partial; anything else (none, stale, longer than the
/// file) is removed so the download starts clean.
async fn resumable_len(part: &Path, size: u64) -> u64 {
    match fs::metadata(part).await {
        Ok(meta) if meta.is_file() && meta.len() <= size => meta.len(),
        _ => {
            discard(part).await;
            0
        }
    }
}

async fn hash_file(path: &Path, hasher: &mut Sha256) -> Result<(), ModelError> {
    let io = ModelError::io(path);
    let mut file = File::open(path).await.map_err(&io)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await.map_err(&io)?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

/// `Content-Range: bytes <start>-<end>/<total>` -> `start`.
fn range_start(response: &Response) -> Option<u64> {
    let value = response
        .headers()
        .get(header::CONTENT_RANGE)?
        .to_str()
        .ok()?;
    value
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

/// Removing a partial that is already gone, or cannot be removed, changes
/// nothing for the caller: the error that led here is the one to report.
async fn discard(part: &Path) {
    let _ = fs::remove_file(part).await;
}
