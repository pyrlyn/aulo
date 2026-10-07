//! What every network adapter shares and none should write twice: the secret
//! handle for an API key and the rule for which endpoints a key may be sent to.
//! Both are pure data and parsing, so they fit a contract crate.

use std::fmt;
use std::net::IpAddr;

use url::Url;

/// A secret resolved from the caller's handle. `Debug` never prints it, so a
/// config logged or dumped in a panic cannot leak it.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret itself, for the one place that must send it (an auth header,
    /// or a client secret minted from it). Never log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

/// A validated endpoint and whether it is on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub url: Url,
    pub local: bool,
}

/// `base_url` plus `path`, accepted on `secure` or, for a machine-local host or
/// a request that carries no key, on `plain`: a key over an unencrypted
/// connection to another host is readable on the path. `None` means refused;
/// the caller picks the error, since each crate has its own.
#[must_use]
pub fn checked_endpoint(
    base_url: &str,
    path: &str,
    has_key: bool,
    (secure, plain): (&str, &str),
) -> Option<Endpoint> {
    let url = Url::parse(&format!("{}{path}", base_url.trim_end_matches('/'))).ok()?;
    let local = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    let scheme = url.scheme();
    (scheme == secure || (scheme == plain && (local || !has_key)))
        .then_some(Endpoint { url, local })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: (&str, &str) = ("wss", "ws");

    fn check(base: &str, has_key: bool) -> Option<Endpoint> {
        checked_endpoint(base, "/x", has_key, WS)
    }

    #[test]
    fn the_secure_scheme_is_always_accepted() {
        let endpoint = check("wss://api.example.com/v1/", true).unwrap();
        assert_eq!(endpoint.url.as_str(), "wss://api.example.com/v1/x");
        assert!(!endpoint.local);
    }

    #[test]
    fn the_plain_scheme_needs_this_machine_or_no_key() {
        for host in ["localhost", "127.0.0.1", "[::1]"] {
            assert!(check(&format!("ws://{host}:9"), true).unwrap().local);
        }
        assert!(check("ws://example.com", true).is_none());
        assert!(check("ws://example.com", false).is_some());
    }

    #[test]
    fn other_schemes_and_garbage_are_refused() {
        assert!(check("https://example.com", false).is_none());
        assert!(check("not a url", false).is_none());
    }

    #[test]
    fn the_key_is_redacted_in_debug_output() {
        assert!(!format!("{:?}", ApiKey::new("sk-secret")).contains("sk-secret"));
    }
}
