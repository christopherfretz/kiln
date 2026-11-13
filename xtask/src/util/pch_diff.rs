//! The PCH differential (WP-1036, `spec/harness.md` "PCH differential"),
//! shared by `cargo xtask test --pch-diff` and `cargo xtask libcxx
//! --pch-diff`: a test's leading `#include`s are built into a precompiled
//! header, and the test is compiled again with `-include-pch` of it. The
//! result must be the one without the PCH.

/// Which consumer path the PCH is read by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PchDiff {
    /// `-fkiln-pch-image=load`: the declaration image where the producer
    /// wrote one, else the replay it falls back to.
    Load,
    /// `-fno-kiln-pch-image`: the header is replayed (the fallback path).
    Replay,
    /// `-fkiln-pch-image=check`: kiln compiles with the image and with
    /// replay and reports the first difference itself.
    Check,
    /// EXPERIMENTAL (WP-1383): `-fkiln-pch-snapshot=load`, the
    /// whole-compiler snapshot the producer wrote with
    /// `-fkiln-pch-snapshot=write` ([`SNAPSHOT_PRODUCER_FLAG`]), else the
    /// replay it falls back to.
    Snapshot,
}

impl PchDiff {
    /// `load`, `replay` or `check`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "load" => Some(Self::Load),
            "replay" => Some(Self::Replay),
            "check" => Some(Self::Check),
            "snapshot" => Some(Self::Snapshot),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Replay => "replay",
            Self::Check => "check",
            Self::Snapshot => "snapshot",
        }
    }

    /// The consumer's flag. The producer always writes the image
    /// (`-fkiln-pch-image`), so `load` and `replay` read the same file.
    pub fn consumer_flag(self) -> &'static str {
        match self {
            Self::Load => "-fkiln-pch-image=load",
            Self::Replay => "-fno-kiln-pch-image",
            Self::Check => "-fkiln-pch-image=check",
            Self::Snapshot => "-fkiln-pch-snapshot=load",
        }
    }
}

/// The producer's flags beyond the test's own: the image and the `-v`
/// note that names the path a consumer takes ([`path_note`]).
pub const PRODUCER_FLAGS: [&str; 2] = ["-fkiln-pch-image", "-v"];

/// [`PchDiff::Snapshot`]'s producer flag, beyond [`PRODUCER_FLAGS`].
pub const SNAPSHOT_PRODUCER_FLAG: &str = "-fkiln-pch-snapshot=write";

/// The environment variable that makes a snapshot consumer print its
/// `snapshot-timing:` lines: under [`PchDiff::Snapshot`] the
/// `snapshot-timing: restore` line tells a restore from a replay.
pub const SNAPSHOT_TIMING_ENV: &str = "KILN_SNAP_TIMING";

/// Under [`PchDiff::Snapshot`], the path the consumer took (its stderr
/// `with`, the producer's `producer`): `snapshot`, or why it replayed.
pub fn snapshot_path(producer: &str, with: &str) -> String {
    if with.lines().any(|l| l.starts_with("snapshot-timing: restore ")) {
        return "snapshot".to_owned();
    }
    let why = producer.lines().find_map(|l| {
        let (_, rest) = l.split_once("': snapshot not written: ")?;
        Some(rest.to_owned())
    });
    match why {
        Some(w) => format!("replay: no snapshot: {w}"),
        None => "replay: the snapshot was not used".to_owned(),
    }
}

/// The `#include` lines that open `source`, before its first line that
/// is neither blank, a comment, nor an `#include <...>`/`#include "..."`
/// (a `#define`, a declaration, an `#if`, a line continued with `\`):
/// what a PCH of the test's prefix can hold without changing the test's
/// meaning, each with its 0-based line number. Empty when the test does
/// not open with an `#include`.
pub fn leading_includes(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_block = false;
    for (n, line) in source.lines().enumerate() {
        let mut t = line.trim();
        if in_block {
            let Some(i) = t.find("*/") else { continue };
            in_block = false;
            t = t[i + 2..].trim();
        }
        while let Some(rest) = t.strip_prefix("/*") {
            if let Some(i) = rest.find("*/") {
                t = rest[i + 2..].trim();
            } else {
                in_block = true;
                t = "";
            }
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        let include = t
            .strip_prefix('#')
            .map(str::trim_start)
            .and_then(|r| r.strip_prefix("include"))
            .map(str::trim_start)
            .is_some_and(|r| r.starts_with('<') || r.starts_with('"'));
        if !include || t.ends_with('\\') || t.contains("/*") {
            break;
        }
        out.push((n, t));
    }
    out
}

/// The header of [`leading_includes`], one per line.
pub fn header_text(includes: &[(usize, &str)]) -> String {
    let mut s = String::new();
    for (_, i) in includes {
        s.push_str(i);
        s.push('\n');
    }
    s
}

/// `source` with the lines of [`leading_includes`] emptied: what the
/// consumer compiles, as a PCH user drops the `#include`s the PCH holds
/// (a header without a guard would otherwise be defined twice). Every
/// other line keeps its number.
pub fn without_includes(source: &str, includes: &[(usize, &str)]) -> String {
    let mut out = String::with_capacity(source.len());
    for (n, line) in source.split_inclusive('\n').enumerate() {
        if includes.iter().any(|&(i, _)| i == n) {
            out.push_str(if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            });
        } else {
            out.push_str(line);
        }
    }
    out
}

/// The path the producer's `-v` note says a `load` consumer takes:
/// `image (N records)` or `replay: <why>`; `None` without a note.
pub fn path_note(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|l| {
        let (_, rest) = l.split_once("': consumers ")?;
        if let Some(n) = rest.strip_prefix("use its declaration image (") {
            return Some(format!("image ({n}"));
        }
        rest.strip_prefix("replay the header: ").map(|why| format!("replay: {why}"))
    })
}

/// `stderr` without what may differ between a compile that includes its
/// headers and one that reads them from a PCH: the include stack (`In
/// file included from` lines, which name the PCH's header), the `N
/// warnings generated.` count, and kiln's `-v` notes about the PCH.
pub fn diag_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| {
            !(l.starts_with("In file included from ")
                || l.starts_with("                 from ")
                || l.ends_with(" generated.")
                || l.starts_with("note: precompiled header '")
                || l.starts_with("snapshot-timing: "))
        })
        .collect()
}

/// Whether the diagnostics of a compile with the PCH (`with`, its
/// producer's `producer`) are those without it (`plain`): equal, or the
/// producer's followed by the consumer's (a consumer that loads the
/// image does not diagnose the header again, as clang's does not).
pub fn same_diags(plain: &str, producer: &str, with: &str) -> bool {
    let (plain, with) = (diag_lines(plain), diag_lines(with));
    if plain == with {
        return true;
    }
    let both: Vec<&str> = diag_lines(producer_diags(producer)).into_iter().chain(with).collect();
    plain == both
}

/// The producer's diagnostics: its stderr after the `-v` banner (which
/// ends with `End of search list.`).
pub fn producer_diags(stderr: &str) -> &str {
    match stderr.find("End of search list.\n") {
        Some(i) => &stderr[i + "End of search list.\n".len()..],
        None => stderr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading() {
        let src = "//@ expect: run\n/* a\n b */\n#include <vector>\n  # include \"x.h\" // c\n\n#include <map>\n#define X\n#include <set>\n";
        let inc = leading_includes(src);
        assert_eq!(
            inc,
            [(3, "#include <vector>"), (4, "# include \"x.h\" // c"), (6, "#include <map>")]
        );
        assert_eq!(
            without_includes(src, &inc),
            "//@ expect: run\n/* a\n b */\n\n\n\n\n#define X\n#include <set>\n"
        );
        assert!(leading_includes("int x;\n#include <vector>\n").is_empty());
        assert!(leading_includes("#include_next <x>\n").is_empty());
        assert!(leading_includes("#include MACRO\n").is_empty());
        assert_eq!(leading_includes("/* c */ #include <a>\n#if 1\n"), [(0, "#include <a>")]);
    }

    #[test]
    fn notes_and_diags() {
        let n =
            "note: precompiled header 'p.pch': consumers use its declaration image (12 records)\n";
        assert_eq!(path_note(n).as_deref(), Some("image (12 records)"));
        let r = "note: precompiled header 'p.pch': consumers replay the header: a lambda\n";
        assert_eq!(path_note(r).as_deref(), Some("replay: a lambda"));
        assert_eq!(path_note("x\n"), None);
        let plain = "In file included from t.cpp:1:\nh.h:1:5: warning: w\nt.cpp:3:1: error: e\n2 warnings generated.\n";
        let producer = "kiln version 1\n #include <...> search starts here:\n /usr/include\nEnd of search list.\nIn file included from p.h:1:\nh.h:1:5: warning: w\n1 warning generated.\n";
        assert!(same_diags(plain, producer, "t.cpp:3:1: error: e\n"));
        assert!(same_diags(
            plain,
            producer,
            "In file included from p.h:1:\nh.h:1:5: warning: w\nt.cpp:3:1: error: e\n"
        ));
        assert!(!same_diags(plain, producer, "t.cpp:3:2: error: e\n"));
        assert!(same_diags(plain, producer, "snapshot-timing: restore x\nt.cpp:3:1: error: e\n"));
    }

    #[test]
    fn snapshot_paths() {
        assert_eq!(snapshot_path("", "snapshot-timing: restore Ast 1 ms\n"), "snapshot");
        let p = "note: precompiled header 'p.pch': snapshot not written: Sema.ctx: x\n";
        assert_eq!(snapshot_path(p, ""), "replay: no snapshot: Sema.ctx: x");
        assert_eq!(snapshot_path("", ""), "replay: the snapshot was not used");
    }
}
