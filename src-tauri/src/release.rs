//! Release metadata for the running build.

use serde::Serialize;

/// `owner/name` of the GitHub repository that publishes releases.
///
/// Set it at build time, e.g.
/// `WHARA_REPOSITORY=you/WhaRaRust cargo tauri build`.
/// When unset the updater stays idle instead of querying a placeholder.
pub const REPOSITORY: Option<&str> = option_env!("WHARA_REPOSITORY");

/// Version of the compiled binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human-readable line used in the preferences footer and menu.
#[derive(Debug, Clone, Serialize)]
pub struct Banner {
    pub version: &'static str,
    pub repository: Option<&'static str>,
}

/// Build a banner describing the running binary.
pub fn banner() -> Banner {
    Banner {
        version: VERSION,
        repository: REPOSITORY,
    }
}

/// Parse a tag such as `v1.2.3` or `1.2.3` into a comparable version.
pub fn parse(tag: &str) -> Option<semver::Version> {
    let trimmed = tag.trim().trim_start_matches(['v', 'V']);
    semver::Version::parse(trimmed).ok()
}

/// True when `candidate` is a strictly newer release than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        // Unparseable tags are never treated as an upgrade.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parse_with_and_without_prefix() {
        assert_eq!(parse("v1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(parse("1.2.3").unwrap().to_string(), "1.2.3");
        assert!(parse("nightly").is_none());
    }

    #[test]
    fn newer_release_is_detected() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
