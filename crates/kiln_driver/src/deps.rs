//! Make-style dependency output (`-M`, `-MM`, `-MD`, `-MMD`, `-MF`, `-MT`,
//! `-MQ`, `-MP`): clang's `DependencyCollector` and
//! `DependencyFileGenerator` (`spec/driver.md` "Dependency output").
//! Shared by `-E` and, for `-MD` / `-MMD`, by `-fsyntax-only`, `-S`, `-c`
//! and linking: the list's setup ([`DepList::for_args`]), the rule's text
//! ([`deps_text`]) and where it goes ([`dep_destination`]).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use kiln_diag::DiagOptions;
use kiln_pp::{FileChange, IncludeRequest, PpCallbacks, ResolvedInclude, SearchEvent};
use kiln_source::{FileId, SourceManager};

use crate::args::{Args, DepHeaders};
use crate::{Io, report_output_error, write_output};

/// clang's `MaxColumns` for the dependency file.
const MAX_COLUMNS: usize = 75;

/// The files a translation unit depends on, in the order first seen.
#[derive(Debug, Default)]
pub struct DepList {
    /// List system headers too (`-M` / `-MD`).
    pub system: bool,
    files: Vec<String>,
    seen: HashSet<String>,
    /// The listed files added as system headers (a PCH records the flag
    /// of each of its input files, WP-1023).
    system_files: HashSet<String>,
    /// An `#include` named a file that was not found (clang's
    /// `SeenMissingHeader`: no dependency file is written).
    missing: bool,
    /// `-MG`: a missing header is listed by its spelled name instead
    /// (clang `AddMissingHeaderDeps`).
    pub generated: bool,
}

impl DepList {
    /// An empty list.
    pub fn new(system: bool) -> DepList {
        DepList { system, ..DepList::default() }
    }

    /// The list `args` asks for (`-M`, `-MM`, `-MD`, `-MMD`), if any.
    /// An `-include-pch` file's input files come first, as clang lists
    /// them when it reads the PCH (WP-1023).
    pub fn for_args(args: &Args) -> Option<DepList> {
        args.deps.headers().map(|h| {
            let mut list = DepList {
                generated: args.deps.missing_generated,
                ..DepList::new(h == DepHeaders::All)
            };
            for (f, system) in &args.pch.deps {
                list.add(f, *system);
            }
            list
        })
    }

    /// Whether an `#include` failed to find its file.
    pub fn saw_missing(&self) -> bool {
        self.missing
    }

    /// clang `DependencyCollector::maybeAddDependency`.
    pub fn add(&mut self, name: &str, system: bool) {
        if (system && !self.system) || is_special(name) {
            return;
        }
        let name = remove_dots(name);
        if self.seen.insert(name.clone()) {
            if system {
                self.system_files.insert(name.clone());
            }
            self.files.push(name);
        }
    }

    /// Whether the listed `name` was added as a system header.
    pub fn is_system(&self, name: &str) -> bool {
        self.system_files.contains(name)
    }

    /// The files.
    pub fn files(&self) -> &[String] {
        &self.files
    }
}

/// `<built-in>`, `<command line>`, `<stdin>`, ...
fn is_special(name: &str) -> bool {
    name.starts_with('<') && name.ends_with('>')
}

/// `llvm::sys::path::remove_dots` without `..` removal: drops `./`
/// components (and a leading `./`) and repeated separators.
fn remove_dots(name: &str) -> String {
    let absolute = name.starts_with('/');
    let parts: Vec<&str> = name.split('/').filter(|c| !c.is_empty() && *c != ".").collect();
    let mut s = String::with_capacity(name.len());
    if absolute {
        s.push('/');
    }
    s.push_str(&parts.join("/"));
    if s.is_empty() { name.to_owned() } else { s }
}

/// A [`PpCallbacks`] that records entered and skipped files into a shared
/// [`DepList`], each by the path its `#include` found it by (clang's
/// `FileEntryRef` name; a file reached through a symlink and through its
/// real path is listed under both).
#[derive(Debug)]
pub struct DepCallbacks<'a> {
    /// The list.
    pub list: Rc<RefCell<DepList>>,
    /// The source manager the file ids refer to.
    pub sm: &'a SourceManager,
    /// The file the last `#include` found and the path it found it by,
    /// until the file is entered.
    pending: Option<(FileId, String)>,
}

impl<'a> DepCallbacks<'a> {
    /// Callbacks recording into `list`.
    pub fn new(list: Rc<RefCell<DepList>>, sm: &'a SourceManager) -> Self {
        DepCallbacks { list, sm, pending: None }
    }
}

impl PpCallbacks for DepCallbacks<'_> {
    fn file_changed(&mut self, change: FileChange) {
        if let FileChange::Enter { file, system, .. } = change {
            let path = match self.pending.take() {
                Some((f, path)) if f == file => path,
                _ => self.sm.file_name(file).to_owned(),
            };
            self.list.borrow_mut().add(&path, system);
        }
    }

    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        match result {
            None => {
                let mut list = self.list.borrow_mut();
                if list.generated {
                    // clang `DependencyFileGenerator::sawDependency` with
                    // `IsMissing`: listed as a user header.
                    list.add(&req.name, false);
                } else {
                    list.missing = true;
                }
            }
            Some(r) => self.pending = r.path.clone().map(|p| (r.file, p)),
        }
    }

    fn file_skipped(&mut self, r: &ResolvedInclude, system: bool) {
        self.pending = None;
        let path = r.path.as_deref().unwrap_or_else(|| self.sm.file_name(r.file));
        self.list.borrow_mut().add(path, system);
    }
}

/// Records a header-search event (`__has_include`, `#embed`).
pub fn on_search_event(list: &RefCell<DepList>, e: SearchEvent<'_>) {
    match e {
        SearchEvent::HasInclude { path, characteristic, .. } => {
            list.borrow_mut().add(path, characteristic.is_system());
        }
        SearchEvent::Embed { path } => list.borrow_mut().add(path, false),
    }
}

/// clang `PrintFilename` (Make format).
pub fn escape_filename(name: &str) -> String {
    let b = name.as_bytes();
    let mut s = String::with_capacity(name.len());
    for (i, c) in name.char_indices() {
        match c {
            '#' => s.push('\\'),
            ' ' => {
                s.push('\\');
                let mut j = i;
                while j > 0 && b[j - 1] == b'\\' {
                    s.push('\\');
                    j -= 1;
                }
            }
            '$' => s.push('$'),
            _ => {}
        }
        s.push(c);
    }
    s
}

/// clang `QuoteTarget` (`-MQ`, and the driver's default target).
pub fn quote_target(t: &str) -> String {
    let b = t.as_bytes();
    let mut s = String::with_capacity(t.len());
    for (i, c) in t.char_indices() {
        match c {
            ' ' | '\t' => {
                let mut j = i;
                while j > 0 && b[j - 1] == b'\\' {
                    s.push('\\');
                    j -= 1;
                }
                s.push('\\');
            }
            '$' => s.push('$'),
            '#' => s.push('\\'),
            _ => {}
        }
        s.push(c);
    }
    s
}

/// clang `DependencyFileGenerator::outputDependencyFile`. `targets` are
/// already quoted as needed.
pub fn render(targets: &[String], files: &[String], phony: bool) -> String {
    let mut out = String::new();
    let mut columns = 0usize;
    for t in targets {
        let n = t.len();
        if columns == 0 {
            columns += n;
        } else if columns + n + 2 > MAX_COLUMNS {
            columns = n + 2;
            out.push_str(" \\\n  ");
        } else {
            columns += n + 1;
            out.push(' ');
        }
        out.push_str(t);
    }
    out.push(':');
    columns += 1;
    for f in files {
        let n = f.len();
        if columns + (n + 1) + 2 > MAX_COLUMNS {
            out.push_str(" \\\n ");
            columns = 2;
        }
        out.push(' ');
        out.push_str(&escape_filename(f));
        columns += n + 1;
    }
    out.push('\n');
    if phony {
        for f in files.iter().skip(1) {
            out.push_str(&escape_filename(f));
            out.push_str(":\n");
        }
    }
    out
}

/// `foo/bar.cpp` -> `bar.o` (clang's default target without `-o`).
pub fn default_target(input: &str) -> String {
    let base = input.rsplit('/').next().unwrap_or(input);
    let stem = match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    };
    format!("{stem}.o")
}

/// The `.d` file of `-MD` / `-MMD` without `-MF`: the `-o` file with its
/// extension replaced, else the input's stem in the working directory.
pub fn default_dep_file(output: Option<&str>, input: &str) -> String {
    let Some(o) = output else {
        let t = default_target(input);
        return format!("{}.d", &t[..t.len() - 2]);
    };
    let slash = o.rfind('/').map_or(0, |i| i + 1);
    match o[slash..].rfind('.') {
        Some(i) if i > 0 => format!("{}.d", &o[..slash + i]),
        _ => format!("{o}.d"),
    }
}

/// Where a dependency rule goes.
pub(crate) enum DepDest {
    /// With the main output (`-M`, `-MM`, `-MF -`).
    Main,
    /// A file.
    File(String),
}

/// clang `RenderDependencyOptions`'s dependency file: `-MF`, else (for
/// `-MD` / `-MMD`) the `-o` file with its extension replaced by `.d`,
/// else `<input stem>.d` in the working directory. `-M` / `-MM` write
/// the rule as the main output.
pub(crate) fn dep_destination(args: &Args, input: &str) -> DepDest {
    if args.deps.only.is_some() {
        return DepDest::Main;
    }
    match args.deps.file.as_deref() {
        Some("-") => DepDest::Main,
        Some(f) => DepDest::File(f.to_owned()),
        None => DepDest::File(default_dep_file(args.output.as_deref(), input)),
    }
}

/// The rule of `input` over `files`: the `-MT` / `-MQ` targets in order,
/// else the `-o` value (not for `-M` / `-MM`), else `<input stem>.o`,
/// Make-quoted; `-MP` adds a phony rule per header.
pub(crate) fn deps_text(args: &Args, input: &str, files: &[String]) -> String {
    let targets: Vec<String> = if args.deps.targets.is_empty() {
        let t = match args.output.as_deref() {
            Some(o) if args.deps.only.is_none() => o.to_owned(),
            _ => default_target(input),
        };
        vec![quote_target(&t)]
    } else {
        args.deps
            .targets
            .iter()
            .map(|(t, quote)| if *quote { quote_target(t) } else { t.clone() })
            .collect()
    };
    render(&targets, files, args.deps.phony)
}

/// `-MD` / `-MMD` with `-fsyntax-only`, `-S`, `-c` or a link: writes the
/// rule of `input` once its preprocessing finished, whatever later
/// errors the translation unit has (clang and GCC keep the file then),
/// unless an `#include` was not found (clang `SeenMissingHeader`).
/// `-MF -` writes it to standard output. `false` after reporting an
/// output error.
pub(crate) fn write_side_file(
    args: &Args,
    input: &str,
    list: &DepList,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> bool {
    if list.saw_missing() {
        return true;
    }
    let text = deps_text(args, input, list.files());
    let path = match dep_destination(args, input) {
        DepDest::Main => "-".to_owned(),
        DepDest::File(f) => f,
    };
    match write_output(Some(&path), &text, &mut io.stdout) {
        Ok(()) => true,
        Err(e) => {
            report_output_error(diag_opts, &io.stderr, &path, &e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(escape_filename("a b#c$d"), "a\\ b\\#c$$d");
        assert_eq!(escape_filename("a\\ b"), "a\\\\\\ b");
        assert_eq!(quote_target("a b$#"), "a\\ b$$\\#");
    }

    #[test]
    fn wrapping_and_phony() {
        let long = format!("/tmp/claude-0/wp14/long/{}.h", "x".repeat(55));
        let files: Vec<String> = vec!["t.cpp".into(), long.clone(), "b.h".into()];
        let s = render(&["t.o".into()], &files, true);
        assert_eq!(s, format!("t.o: t.cpp \\\n  {long} \\\n  b.h\n{long}:\nb.h:\n"));
    }

    #[test]
    fn list_filters_and_dedups() {
        let mut l = DepList::new(false);
        l.add("<built-in>", false);
        l.add("./a.h", false);
        l.add("a.h", false);
        l.add("/usr/include/s.h", true);
        assert_eq!(l.files(), ["a.h"]);
    }

    #[test]
    fn defaults() {
        assert_eq!(default_target("dir/t.cpp"), "t.o");
        assert_eq!(default_target("noext"), "noext.o");
        assert_eq!(default_dep_file(Some("out/x.i"), "t.cpp"), "out/x.d");
        assert_eq!(default_dep_file(None, "dir/t.cpp"), "t.d");
    }
}
