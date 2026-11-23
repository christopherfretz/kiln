//! The environment variables the driver reads (spec/driver.md
//! "Environment variables"): the process's for the `kiln` executable, or
//! a fixed set a library entry is given ([`crate::front_end`]), so that
//! an entry never reads the process environment.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::sync::Arc;

/// Where the driver reads `KILN_*`, `PATH`, `SOURCE_DATE_EPOCH`, `HOME`
/// and `XDG_CACHE_HOME` from: the process environment (the default), or
/// a fixed set of variables. A fixed set is also the whole environment
/// of the programs the driver runs to locate its toolchain (the GCC
/// driver's `-print-search-dirs`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Environment(Option<Arc<BTreeMap<OsString, OsString>>>);

impl Environment {
    /// The process environment, read when a variable is needed.
    pub fn process() -> Environment {
        Environment(None)
    }

    /// Exactly `vars` (a later value of a name replaces an earlier one);
    /// every other variable is unset.
    pub fn fixed<K: Into<OsString>, V: Into<OsString>>(
        vars: impl IntoIterator<Item = (K, V)>,
    ) -> Environment {
        Environment(Some(Arc::new(vars.into_iter().map(|(k, v)| (k.into(), v.into())).collect())))
    }

    /// Whether this is the process environment.
    pub fn is_process(&self) -> bool {
        self.0.is_none()
    }

    /// The variable `name`, as [`std::env::var_os`].
    pub fn var_os(&self, name: &str) -> Option<OsString> {
        match &self.0 {
            None => std::env::var_os(name),
            Some(vars) => vars.get(OsStr::new(name)).cloned(),
        }
    }

    /// The variable `name` when it is set and valid Unicode, as
    /// [`std::env::var`]`(name).ok()`.
    pub fn var(&self, name: &str) -> Option<String> {
        self.var_os(name).and_then(|v| v.into_string().ok())
    }

    /// A command running `program` in this environment: the process's,
    /// or exactly the fixed set (`PATH` included, which also finds
    /// `program` when it names no directory).
    pub(crate) fn command(&self, program: impl AsRef<OsStr>) -> std::process::Command {
        let mut cmd = std::process::Command::new(program);
        if let Some(vars) = &self.0 {
            cmd.env_clear().envs(vars.iter());
        }
        cmd
    }
}

/// Whether `KILN_WREF_RAW` is set in `env`: the written-reference dump
/// prefixes each line with the name's raw location (a developer switch).
pub(crate) fn wref_raw(env: &Environment) -> bool {
    env.var_os("KILN_WREF_RAW").is_some()
}

/// The names only: values may be secrets.
impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            None => f.write_str("Environment::process()"),
            Some(vars) => f.debug_tuple("Environment::fixed").field(&vars.keys()).finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_hides_the_process_environment() {
        let e = Environment::fixed([("KILN_X", "1"), ("PATH", "/nowhere"), ("KILN_X", "2")]);
        assert_eq!(e.var("KILN_X").as_deref(), Some("2"));
        assert_eq!(e.var("PATH").as_deref(), Some("/nowhere"));
        // `HOME` is set in any test environment kiln runs in, but not here.
        assert_eq!(e.var_os("HOME"), None);
        assert!(!e.is_process() && Environment::default().is_process());
        assert_eq!(format!("{e:?}"), r#"Environment::fixed(["KILN_X", "PATH"])"#);
    }
}
