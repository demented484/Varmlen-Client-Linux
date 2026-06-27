//! Split-tunnel selection — shared by the xray config generator and the
//! OS-level per-app router (the helper).
//!
//! One `mode` applies to BOTH the apps and sites lists:
//!   - selective = whitelist (only listed apps/sites are tunneled; default direct)
//!   - general   = blacklist (everything is tunneled; listed entries are
//!     exceptions that stay direct)
//!
//! Both dimensions are enforced inside xray's routing: the app dimension via
//! xray's native `process` matcher (the native tun preserves each app's local
//! socket, so xray resolves the owning process via /proc), the site dimension
//! via `domain` rules.

use serde::Deserialize;

/// Split-tunnel selection passed from the UI (only enabled entries).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SplitInput {
    /// "selective" | "general". Empty string is treated as "general" so an
    /// uninitialised input doesn't accidentally cut the user's network.
    #[serde(default)]
    pub mode: String,
    /// Process / binary names of enabled apps.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Enabled site patterns (e.g. "example.com" or "*.example.com").
    #[serde(default)]
    pub sites: Vec<String>,
}

impl SplitInput {
    /// selective = whitelist (only listed entries are tunneled).
    pub fn is_selective(&self) -> bool {
        self.mode == "selective"
    }

    /// Enabled, non-empty app/process names.
    pub fn enabled_apps(&self) -> Vec<String> {
        self.apps.iter().filter(|a| !a.is_empty()).cloned().collect()
    }
}
