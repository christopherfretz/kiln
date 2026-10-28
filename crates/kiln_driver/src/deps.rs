//! Make-style dependency output (`-M`, `-MM`, `-MD`, `-MMD`, `-MF`, `-MT`,
//! `-MQ`, `-MP`): clang's `DependencyCollector` and
//! `DependencyFileGenerator` (`spec/driver.md` "Dependency output").

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use kiln_pp::{FileChange, PpCallbacks, SearchEvent};
use kiln_source::SourceManager;

/// clang's `MaxColumns` for the dependency file.
const MAX_COLUMNS: usize = 75;

/// The files a translation unit depends on, in the order first seen.
#[derive(Debug, Default)]
pub struct DepList {
    /// List system headers too (`-M` / `-MD`).
    pub system: bool,
    files: Vec<String>,
    seen: HashSet<String>,
}

impl DepList {
    /// An empty list.
    pub fn new(system: bool) -> DepList {
        DepList { system, ..DepList::default() }
    }

    /// clang `DependencyCollector::maybeAddDependency`.
    pub fn add(&mut self, name: &str, system: bool) {
        if (system && !self.system) || is_special(name) {
            return;
        }
        let name = remove_dots(name);
        if self.seen.insert(name.clone()) {
            self.files.push(name);
        }
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

/// A [`PpCallbacks`] that records entered files into a shared [`DepList`].
#[derive(Debug)]
pub struct DepCallbacks<'a> {
    /// The list.
    pub list: Rc<RefCell<DepList>>,
    /// The source manager the file ids refer to.
    pub sm: &'a SourceManager,
}

impl PpCallbacks for DepCallbacks<'_> {
    fn file_changed(&mut self, change: FileChange) {
        if let FileChange::Enter { file, system, .. } = change {
            self.list.borrow_mut().add(self.sm.file_name(file), system);
        }
    }
}

/// Records a header-search event (`__has_include`, `#embed`).
pub fn on_search_event(list: &RefCell<DepList>, sm: &SourceManager, e: SearchEvent<'_>) {
    match e {
        SearchEvent::HasInclude { file, characteristic } => {
            list.borrow_mut().add(sm.file_name(file), characteristic.is_system());
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
