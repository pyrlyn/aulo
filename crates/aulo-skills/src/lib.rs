//! Skills (spec §9, T11.8).
//!
//! `agent-ext` discovers `SKILL.md` files. The prompt receives one index
//! line per skill. The body stays on disk until [`Skills::load_body`].

use std::path::Path;

use agent_ext::skills::{self, SkillTool};

pub use agent_ext::skills::{SkillNotFound, SkillToolSpec};

/// Directory name under a project and the skills folder under the aulo home.
const APP_DIR: &str = ".aulo";

/// Bytes of a skill body returned to the model. The rest is cut.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Skills found for one home and an optional project. A broken file is a
/// notice, not a failure.
pub struct Skills {
    tool: SkillTool,
    index: String,
    notices: Vec<String>,
}

impl std::fmt::Debug for Skills {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Skills")
            .field("index", &self.index)
            .field("notices", &self.notices)
            .finish_non_exhaustive()
    }
}

impl Skills {
    /// Scans `<home>/skills/*/SKILL.md` and `<project>/.aulo/skills/*/SKILL.md`.
    /// Later directories replace an earlier skill of the same name.
    #[must_use]
    pub fn load(home: Option<&Path>, project: Option<&Path>) -> Self {
        let dirs = skills::skill_dirs(APP_DIR, home, None, project);
        let found = skills::discover(&dirs);
        let index = skills::index(&found.skills);
        let tool = SkillTool::new(found.skills);
        Self {
            tool,
            index,
            notices: found.notices,
        }
    }

    /// The prompt fragment. Empty when there are no skills. Bodies are not
    /// in this string.
    #[must_use]
    pub fn index(&self) -> &str {
        &self.index
    }

    /// Skills that were skipped, and why. Paths only, never file contents.
    #[must_use]
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    /// What the tool registry records. `deferred` is set, so the body is not
    /// part of the tool list.
    #[must_use]
    pub fn spec(&self) -> SkillToolSpec {
        self.tool.spec()
    }

    /// The body for `name`, capped at [`MAX_BODY_BYTES`].
    ///
    /// # Errors
    ///
    /// [`SkillNotFound`] when `name` is empty, too long, or not a loaded skill.
    pub fn load_body(&self, name: &str) -> Result<String, SkillNotFound> {
        if name.is_empty() || name.len() > 64 {
            return Err(SkillNotFound);
        }
        let output = self.tool.call(&serde_json::json!({ "name": name }))?;
        Ok(cap_bytes(&output.text, MAX_BODY_BYTES))
    }
}

fn cap_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text[..end].to_string();
    out.push_str("\n[truncated]");
    out
}
