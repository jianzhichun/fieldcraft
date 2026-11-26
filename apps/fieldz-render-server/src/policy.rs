//! The read-only policy: the host's reason, applied to the engine's process-wide flag.
//!
//! `--read-only <reason>` or `FIELDCRAFT_READ_ONLY=<reason>`; the flag itself lives in
//! `fieldcraft_engine::read_only` because every command, on whatever thread, checks it there.

/// The environment variable naming the read-only reason.
pub const ENV: &str = "FIELDCRAFT_READ_ONLY";

/// The reason the flag and the environment name, without touching the engine: the command
/// line's wins; an empty one is no reason.
pub fn reason(flag: Option<String>) -> Option<String> {
    flag.or_else(|| std::env::var(ENV).ok()).map(|r| r.trim().to_string()).filter(|r| !r.is_empty())
}

/// Apply the flag to the engine. Returns the reason the session is read-only with.
pub fn apply(flag: Option<String>) -> Option<String> {
    let reason = reason(flag);
    if let Some(reason) = &reason {
        fieldcraft_engine::read_only::set(reason.clone());
    }
    reason
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command line wins; whitespace is not a reason; with neither, the environment (unset
    /// in the usual test run) decides. The engine's flag is not touched here: other tests in
    /// this binary run commands in parallel.
    #[test]
    fn the_flag_wins_and_empty_is_none() {
        assert_eq!(reason(Some("from-flag".into())), Some("from-flag".into()));
        if std::env::var(ENV).is_err() {
            assert_eq!(reason(Some("   ".into())), None);
            assert_eq!(reason(None), None);
        }
    }
}
