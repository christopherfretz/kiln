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

/// What [`snapshot_path`] calls a consumer that restored the snapshot.
pub const RESTORED: &str = "snapshot";

/// Under [`PchDiff::Snapshot`], the path the consumer took (its stderr
/// `with`, the producer's `producer`): [`RESTORED`], or why it replayed:
/// `replay: producer: <why>` when the producer wrote no snapshot (its
/// `-v` note), else `replay: consumer: <why>` (the consumer's last
/// `snapshot-timing: note:` line, printed under [`SNAPSHOT_TIMING_ENV`]),
/// else `replay: consumer: (no reason given)`.
pub fn snapshot_path(producer: &str, with: &str) -> String {
    if with.lines().any(|l| l.starts_with("snapshot-timing: restore ")) {
        return RESTORED.to_owned();
    }
    let why = producer.lines().find_map(|l| {
        let (_, rest) = l.split_once("': snapshot not written: ")?;
        Some(rest.to_owned())
    });
    if let Some(w) = why {
        return format!("replay: producer: {w}");
    }
    let why = with
        .lines()
        .filter_map(|l| l.strip_prefix("snapshot-timing: note: "))
        .rfind(|w| !w.starts_with("matches;") && !w.starts_with("restored;"));
    match why {
        Some(w) => format!("replay: consumer: {}", w.strip_prefix("not used: ").unwrap_or(w)),
        None => "replay: consumer: (no reason given)".to_owned(),
    }
}

/// The replay reason of a [`snapshot_path`] (`None` for [`RESTORED`] and
/// for a path of another mode), with its numbers and paths reduced so a
/// histogram groups like reasons: every run of digits is `N`, and a
/// quoted path (`'...'`) is `'…'`.
pub fn replay_reason(path: &str) -> Option<String> {
    let why = path.strip_prefix("replay: ")?;
    let mut out = String::with_capacity(why.len());
    let mut chars = why.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
            out.push('N');
        } else if c == '\'' {
            let quoted: String = chars.by_ref().take_while(|&d| d != '\'').collect();
            if quoted.contains('/') {
                out.push_str("'…'");
            } else {
                out.push('\'');
                out.push_str(&quoted);
                out.push('\'');
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// One entry of the snapshot replay allow-list ([`AllowList`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowed {
    /// A substring of the replay reason ([`snapshot_path`] without its
    /// `replay: `).
    pub pattern: String,
    pub justification: String,
}

/// The allow-list of `--pch-diff-require-restore` (spec/harness.md "PCH
/// differential"): the reasons a snapshot consumer may replay for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllowList(pub Vec<Allowed>);

/// The allow-list's path, relative to the repository.
pub const ALLOW_LIST: &str = "tests/pch-snapshot-replay-allow.txt";

impl AllowList {
    /// Parses the file's text: one `<pattern> :: <justification>` per
    /// line (both non-empty), `#` comments and blank lines ignored.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let Some((p, j)) = t.split_once(" :: ") else {
                return Err(format!("line {}: not `<pattern> :: <justification>`", n + 1));
            };
            let (p, j) = (p.trim(), j.trim());
            if p.is_empty() || j.is_empty() {
                return Err(format!("line {}: empty pattern or justification", n + 1));
            }
            out.push(Allowed { pattern: p.to_owned(), justification: j.to_owned() });
        }
        Ok(Self(out))
    }

    /// The entry that allows `path` ([`snapshot_path`]): `None` for a
    /// replay no entry covers. A restore needs none.
    pub fn allows(&self, path: &str) -> Option<&Allowed> {
        let why = path.strip_prefix("replay: ")?;
        self.0.iter().find(|a| why.contains(&a.pattern))
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

/// The prefix `--pch-diff-synthetic` gives a test without `#include`s
/// (spec/harness.md "PCH differential"): libstdc++'s whole library, the
/// largest realistic prefix.
pub const SYNTHETIC_HEADER: &str = "#include <bits/stdc++.h>\n";

/// Whether `source` has an `#include`-like directive anywhere (a line
/// whose first non-blank character is `#` followed by `include` or
/// `import`): such a test is not given the synthetic prefix.
pub fn has_include(source: &str) -> bool {
    source.lines().any(|l| {
        l.trim_start().strip_prefix('#').is_some_and(|r| {
            let r = r.trim_start();
            r.starts_with("include") || r.starts_with("import")
        })
    })
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

/// The IR of a compile with or without the PCH, after the corpus's
/// `ir_normalize`, without what depends on the main file's bytes or on
/// raw source locations rather than on the program (WP-1395): a `DIFile`'s
/// `checksum:` (the PCH build's copy of the test has its `#include` lines
/// emptied) and the values of an inline asm's `!srcloc` tuples
/// (`!N = !{i64 M, ...}`, a raw location as clang's, which a PCH shifts
/// in clang too).
pub fn ir_pch_normalize(ir: &str) -> String {
    let mut out = String::with_capacity(ir.len());
    for line in ir.lines() {
        if let Some((head, rest)) = line.split_once(" = !{i64 ")
            && head.starts_with('!')
            && rest.ends_with('}')
            && rest[..rest.len() - 1].split(", i64 ").all(|w| w.parse::<i64>().is_ok())
        {
            out.push_str(head);
            out.push_str(" = !{i64 …}\n");
            continue;
        }
        match line.find("checksum: \"") {
            Some(i) => {
                let tail = &line[i + "checksum: \"".len()..];
                let close = tail.find('"').map_or(tail.len(), |j| j + 1);
                out.push_str(&line[..i]);
                out.push_str("checksum: \"…\"");
                out.push_str(&tail[close.min(tail.len())..]);
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

/// A test binary's output without what differs between two runs of one
/// program: its own path's file name (`plain.out`, `pch.out`,
/// `textual.out`: a sanitizer report names the binary) made one, and a
/// sanitizer report's process id (`==N==`), addresses (`0x...`) and
/// `BuildId:` reduced (address-space layout is randomized).
pub fn run_output_normalize(text: &str) -> String {
    let text = text
        .replace("/plain.out", "/test.out")
        .replace("/textual.out", "/test.out")
        .replace("/pch.out", "/test.out");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(c) = rest.chars().next() {
        let hex_run = |s: &str| s.bytes().take_while(u8::is_ascii_hexdigit).count();
        if let Some(r) = rest.strip_prefix("0x") {
            out.push_str("0x…");
            rest = &r[hex_run(r)..];
        } else if let Some(r) = rest.strip_prefix("BuildId: ") {
            out.push_str("BuildId: …");
            rest = &r[hex_run(r)..];
        } else if let Some(r) = rest.strip_prefix("==")
            && let n = r.bytes().take_while(u8::is_ascii_digit).count()
            && n > 0
            && r[n..].starts_with("==")
        {
            out.push_str("==N==");
            rest = &r[n + 2..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
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
///
/// A snapshot consumer (`-fkiln-pch-snapshot=load`, WP-1394) says under
/// `KILN_SNAP_TIMING` how many warnings and errors the header reported
/// when it was built ([`snapshot_prefix_reported`]); those come first
/// in `plain`, and `with` must be `plain` without them (the
/// consumer-only diagnostics, spec/driver.md "PCH snapshot: behaviour").
pub fn same_diags(plain: &str, producer: &str, with: &str) -> bool {
    let n = snapshot_prefix_reported(with);
    let (plain, with) = (diag_lines(plain), diag_lines(with));
    if plain == with {
        return true;
    }
    if let Some(n) = n {
        return without_leading_diags(&plain, n) == with.as_slice();
    }
    let both: Vec<&str> = diag_lines(producer_diags(producer)).into_iter().chain(with).collect();
    plain == both
}

/// What a restored snapshot's consumer says its header reported:
/// `snapshot-timing: the header reported N diagnostic(s)`.
pub fn snapshot_prefix_reported(stderr: &str) -> Option<usize> {
    stderr.lines().find_map(|l| {
        let rest = l.strip_prefix("snapshot-timing: the header reported ")?;
        rest.split(' ').next()?.parse().ok()
    })
}

/// Whether `l` is a warning's or an error's first line.
fn primary(l: &str) -> bool {
    !l.starts_with(' ')
        && (l.contains(": warning: ") || l.contains(": error: ") || l.contains(": fatal error: "))
}

/// `lines` from its `n + 1`th warning or error on: without the first
/// `n` and their source lines and notes.
fn without_leading_diags<'a, 'b>(lines: &'b [&'a str], n: usize) -> &'b [&'a str] {
    if n == 0 {
        return lines;
    }
    let at =
        lines.iter().enumerate().filter(|(_, l)| primary(l)).nth(n).map_or(lines.len(), |(i, _)| i);
    &lines[at..]
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
        assert!(has_include("int x;\n  #  include_next <x>\n"));
        assert!(!has_include("int x; // #include <x>\n"));
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
        // A snapshot consumer: the header's first diagnostic is not shown
        // again; a later one at the same place (the main file's
        // instantiation) is its own.
        let plain = "h.h:1:5: warning: w\n  1 | x\nh.h:2:1: note: n\nh.h:1:5: warning: w\nt.cpp:3:1: error: e\n";
        let with = "snapshot-timing: the header reported 1 diagnostic(s) when it was built\n";
        assert!(same_diags(
            plain,
            "",
            &format!("h.h:1:5: warning: w\nt.cpp:3:1: error: e\n{with}")
        ));
        assert!(!same_diags(plain, "", &format!("t.cpp:3:1: error: e\n{with}")));
        assert_eq!(snapshot_prefix_reported(with), Some(1));
        assert_eq!(snapshot_prefix_reported("x\n"), None);
    }

    #[test]
    fn snapshot_paths() {
        assert_eq!(snapshot_path("", "snapshot-timing: restore Ast 1 ms\n"), "snapshot");
        let p = "note: precompiled header 'p.pch': snapshot not written: Sema.ctx: x\n";
        assert_eq!(snapshot_path(p, ""), "replay: producer: Sema.ctx: x");
        assert_eq!(snapshot_path("", ""), "replay: consumer: (no reason given)");
        let c = "snapshot-timing: note: matches; restoring it\nsnapshot-timing: note: not used (x); the header is replayed\n";
        assert_eq!(snapshot_path("", c), "replay: consumer: not used (x); the header is replayed");
        let c = "snapshot-timing: note: not used: the command line differs: '-DA' (producer) vs '-DB' (here)\n";
        let p = snapshot_path("", c);
        assert_eq!(
            p,
            "replay: consumer: the command line differs: '-DA' (producer) vs '-DB' (here)"
        );
        assert_eq!(replay_reason(RESTORED), None);
        assert_eq!(
            replay_reason("replay: consumer: '/a/b.h' changed, 12 files").as_deref(),
            Some("consumer: '…' changed, N files")
        );
    }

    #[test]
    fn pch_ir() {
        let a = "!7 = !{i64 710, i64 12}\n!3 = !DIFile(filename: \"a\", checksumkind: CSK_MD5, checksum: \"14e3\")\n!8 = !{!\"auto-init\"}\n";
        let b = "!7 = !{i64 692, i64 1}\n!3 = !DIFile(filename: \"a\", checksumkind: CSK_MD5, checksum: \"0e46\")\n!8 = !{!\"auto-init\"}\n";
        assert_eq!(ir_pch_normalize(a), ir_pch_normalize(b));
        assert!(ir_pch_normalize(a).contains("auto-init"));
        assert_ne!(ir_pch_normalize("!1 = !{i32 7}\n"), ir_pch_normalize("!1 = !{i32 8}\n"));
        assert_eq!(
            run_output_normalize("#0 in f /x/t1/pch.out+0x1f"),
            "#0 in f /x/t1/test.out+0x…"
        );
        assert_eq!(
            run_output_normalize("==57824==ERROR: at pc 0xaaaa (BuildId: f844)\n=x="),
            "==N==ERROR: at pc 0x… (BuildId: …)\n=x="
        );
    }

    #[test]
    fn allow_list() {
        let l = AllowList::parse("# c\n\nproducer: the prefix has errors :: no state to save\n")
            .unwrap();
        assert_eq!(l.0.len(), 1);
        assert!(l.allows("replay: producer: the prefix has errors (3)").is_some());
        assert!(l.allows("replay: consumer: x").is_none());
        assert!(l.allows(RESTORED).is_none());
        assert!(AllowList::parse("x\n").is_err());
        assert!(AllowList::parse("x :: \n").is_err());
    }
}
