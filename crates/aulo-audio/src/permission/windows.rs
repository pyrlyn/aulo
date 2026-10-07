//! Windows privacy settings, read from the capability access manager's consent
//! store. Settings > Privacy & security > Microphone writes `Allow` or `Deny`
//! to these values; reading them is safe and never prompts. Group policy
//! writes the machine-wide copy, which is why both hives are read.

use windows_registry::{CURRENT_USER, Key, LOCAL_MACHINE};

use super::{MicPermission, consent_status};

const MICROPHONE: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
/// The "Let desktop apps access your microphone" switch, which covers an
/// unpackaged program such as aulo.
const DESKTOP_APPS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged";

fn consent(root: &Key, path: &str) -> Option<String> {
    root.open(path).ok()?.get_string("Value").ok()
}

pub(super) fn status() -> MicPermission {
    let values: Vec<Option<String>> = [LOCAL_MACHINE, CURRENT_USER]
        .iter()
        .flat_map(|root| [MICROPHONE, DESKTOP_APPS].map(|path| consent(root, path)))
        .collect();
    consent_status(values.iter().map(Option::as_deref))
}
