//! The environment every session child receives (FR-SESS-007, `docs/11` §1 and §4, ADR 0026 W8).
//!
//! Only these documented variables are injected (`docs/10` §3 item 10); everything else is the
//! environment the daemon inherited, as the user's shell would get it.

use std::ffi::OsString;

use crate::EnvOp;

/// Value of `TERM` on Unix (`docs/11` §1).
pub const TERM: &str = "xterm-256color";

/// Value of `TERM_PROGRAM` (`docs/11` §1).
pub const TERM_PROGRAM: &str = "Ferrum";

/// Environment changes for one session, to append to
/// [`SpawnSpec::env`](crate::SpawnSpec::env) before any per-profile changes.
///
/// Sets `COLORTERM=truecolor`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION` (this crate's version),
/// `FERRUM=1`, `FERRUM_SESSION_ID` and `FERRUM_PROJECT_ID`. On Unix it sets `TERM`; on Windows it
/// removes `TERM`, because Windows tools ignore it and an inherited value would mislead programs
/// that read it (`docs/11` §4, console quirks).
pub fn session_env(session_id: &str, project_id: &str) -> Vec<(OsString, EnvOp)> {
    let set = |name: &str, value: &str| (OsString::from(name), EnvOp::Set(value.into()));
    vec![
        term_entry(),
        set("COLORTERM", "truecolor"),
        set("TERM_PROGRAM", TERM_PROGRAM),
        set("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION")),
        set("FERRUM", "1"),
        set("FERRUM_SESSION_ID", session_id),
        set("FERRUM_PROJECT_ID", project_id),
    ]
}

#[cfg(unix)]
fn term_entry() -> (OsString, EnvOp) {
    ("TERM".into(), EnvOp::Set(TERM.into()))
}

#[cfg(windows)]
fn term_entry() -> (OsString, EnvOp) {
    ("TERM".into(), EnvOp::Remove)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(env: &'a [(OsString, EnvOp)], name: &str) -> Option<&'a EnvOp> {
        env.iter().find(|(key, _)| key == name).map(|(_, op)| op)
    }

    #[test]
    fn injects_the_documented_variables() {
        let env = session_env("s1", "p1");
        let set = |text: &str| Some(EnvOp::Set(text.into()));
        assert_eq!(value(&env, "COLORTERM").cloned(), set("truecolor"));
        assert_eq!(value(&env, "TERM_PROGRAM").cloned(), set("Ferrum"));
        assert_eq!(value(&env, "TERM_PROGRAM_VERSION").cloned(), set(env!("CARGO_PKG_VERSION")));
        assert_eq!(value(&env, "FERRUM").cloned(), set("1"));
        assert_eq!(value(&env, "FERRUM_SESSION_ID").cloned(), set("s1"));
        assert_eq!(value(&env, "FERRUM_PROJECT_ID").cloned(), set("p1"));
        let term = if cfg!(windows) { Some(EnvOp::Remove) } else { set("xterm-256color") };
        assert_eq!(value(&env, "TERM").cloned(), term);
        assert_eq!(env.len(), 7, "only documented variables are injected");
    }
}
