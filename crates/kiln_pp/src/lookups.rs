//! The lookup log (WP-1403, spec/pp.md "Lookup log"): the header lookups
//! a prefix performed, recorded by [`HeaderSearch`](crate::HeaderSearch)
//! and repeated by a PCH snapshot's consumer over its own search list
//! ([`LookupLog::check`], spec/pch.md, "Lookup-tolerant
//! search path").
//!
//! A lookup's result is a function of its key (the name, the form, the
//! includer's directory where the form tries it, the directory an
//! `#include_next` continues after), the search list and the file system.
//! So a consumer whose list gives every recorded key the recorded result
//! looks up the same files the prefix did, in the same order.
//!
//! A lookup of an absolute name (WP-2620) does not use the search list:
//! its result is a function of the file system alone, and it is always
//! asked again ([`LookupLog::check_absolute`]).

use std::collections::{HashMap, HashSet};

use kiln_snap::Snap;

use crate::resolver::{LIST_AFTER_MISSES, Listing, listable};
use crate::search::{Characteristic, HeaderFs, SearchList, join_path};

/// What kind of lookup a [`Lookup`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Snap)]
pub enum LookupForm {
    /// `#include`, `#import`, `__has_include`, and an `_next` form that
    /// starts from the beginning (in the main file, or in a header found
    /// relative to its includer).
    Include,
    /// `#include_next` / `__has_include_next` continuing after the
    /// directory [`Lookup::after`].
    Next,
    /// `#embed` / `__has_embed`.
    Embed,
}

/// The result of a [`Lookup`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Snap)]
pub enum Found {
    /// Nothing was found.
    Missing,
    /// Found relative to the includer's directory ([`Lookup::includer`]).
    Includer,
    /// Found in the search directory [`LookupLog::dirs`]`[dir]`, with
    /// that directory's characteristic.
    Dir {
        /// The directory, an index into [`LookupLog::dirs`].
        dir: u32,
        /// Its characteristic.
        characteristic: Characteristic,
    },
    /// An `#embed` resource read from `path`: relative to the includer's
    /// directory, to the working directory (the name itself), or, for an
    /// absolute name, the name itself.
    Embed {
        /// The path it was read from.
        path: String,
    },
    /// An `#embed` resource found in the `--embed-dir`
    /// [`LookupLog::embed_dirs`]`[index]` (WP-2829).
    EmbedDir {
        /// Its position in [`LookupLog::embed_dirs`].
        index: u32,
    },
    /// Found at the name as written: an absolute name, opened as is
    /// (WP-2620).
    Absolute,
}

/// One recorded lookup: its key and its result.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Snap)]
pub struct Lookup {
    /// The name as written between the delimiters.
    pub name: String,
    /// The form.
    pub form: LookupForm,
    /// `<...>`.
    pub angled: bool,
    /// The includer's directory (an index into [`LookupLog::dirs`]) where
    /// the form tries it first: a `"..."` [`LookupForm::Include`] (`.` for
    /// the predefines buffer) or embed.
    pub includer: Option<u32>,
    /// For [`LookupForm::Next`], the directory the search continues
    /// after (an index into [`LookupLog::dirs`]).
    pub after: Option<u32>,
    /// What was found.
    pub found: Found,
}

/// The lookups a prefix performed, each distinct one once, in the order
/// first seen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Snap)]
pub struct LookupLog {
    /// The directory names the lookups refer to.
    pub dirs: Vec<String>,
    /// The lookups.
    pub lookups: Vec<Lookup>,
    /// The producer's search list, as indices into [`LookupLog::dirs`]:
    /// the directories each lookup searched before its result, which it
    /// did not find the name in.
    pub list: Vec<u32>,
    /// The position in [`LookupLog::list`] where `<...>` lookups start.
    pub angled_start: u32,
    /// The producer's `--embed-dir` list, in order, as indices into
    /// [`LookupLog::dirs`] (WP-2829): an `#embed` searches it after the
    /// includer's and the working directory.
    pub embed_dirs: Vec<u32>,
}

/// The recording side: the log and what deduplicates it.
#[derive(Debug, Default)]
pub(crate) struct Recorder {
    log: LookupLog,
    dir_ids: HashMap<String, u32>,
    seen: HashSet<Lookup>,
}

impl Recorder {
    /// The index of the directory name `d`.
    pub(crate) fn dir(&mut self, d: &str) -> u32 {
        if let Some(&i) = self.dir_ids.get(d) {
            return i;
        }
        let i = u32::try_from(self.log.dirs.len()).unwrap_or(u32::MAX);
        self.log.dirs.push(d.to_owned());
        self.dir_ids.insert(d.to_owned(), i);
        i
    }

    /// Records the search list and the `--embed-dir` list the lookups
    /// use.
    pub(crate) fn start(&mut self, list: &SearchList, embed_dirs: &[String]) {
        self.log.list = list.dirs().iter().map(|d| self.dir(&d.path)).collect();
        self.log.angled_start = u32::try_from(list.angled_start()).unwrap_or(u32::MAX);
        self.log.embed_dirs = embed_dirs.iter().map(|d| self.dir(d)).collect();
    }

    /// Records `l` unless it was recorded before.
    pub(crate) fn add(&mut self, l: Lookup) {
        if self.seen.insert(l.clone()) {
            self.log.lookups.push(l);
        }
    }

    /// The log so far.
    pub(crate) fn take(&mut self) -> LookupLog {
        self.seen.clear();
        self.dir_ids.clear();
        std::mem::take(&mut self.log)
    }
}

impl LookupLog {
    /// Whether every path a result names is absolute (the includer's
    /// directory of a relative hit, the search directory of a hit, an
    /// embed's path): the results then name the same files from any
    /// working directory.
    pub fn absolute(&self) -> bool {
        let abs = |i: u32| self.dir(i).starts_with('/');
        self.lookups.iter().all(|l| match &l.found {
            Found::Missing => true,
            Found::Absolute => l.name.starts_with('/'),
            Found::Includer => l.includer.is_some_and(abs),
            Found::Dir { dir, .. } => abs(*dir),
            Found::Embed { path } => path.starts_with('/'),
            Found::EmbedDir { index } => {
                self.embed_dirs.get(*index as usize).is_some_and(|&d| abs(d))
            }
        })
    }

    /// Checks a decoded log's indices (WP-1396, spec/snap.md "Threat
    /// model"): every directory index (a result's, an includer's, an
    /// `after`, the list's, the `--embed-dir` list's) names one of
    /// [`LookupLog::dirs`], [`LookupLog::angled_start`] is within the
    /// list, and an [`Found::EmbedDir`] within [`LookupLog::embed_dirs`]. A log read from a
    /// file that fails is refused before it is repeated.
    ///
    /// # Errors
    /// What is out of range.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.dirs.len();
        let bad = |what: &str, i: u32| -> Result<(), String> {
            if (i as usize) < n {
                Ok(())
            } else {
                Err(format!("its lookup log names directory {i} of {n} ({what})"))
            }
        };
        for &d in &self.list {
            bad("search list", d)?;
        }
        for &d in &self.embed_dirs {
            bad("--embed-dir list", d)?;
        }
        if self.angled_start as usize > self.list.len() {
            return Err(format!(
                "its lookup log starts the <...> search at {} of {}",
                self.angled_start,
                self.list.len()
            ));
        }
        for l in &self.lookups {
            if let Some(i) = l.includer {
                bad("includer", i)?;
            }
            if let Some(a) = l.after {
                bad("include_next", a)?;
            }
            if let Found::Dir { dir, .. } = l.found {
                bad("result", dir)?;
            }
            if let Found::EmbedDir { index } = l.found
                && index as usize >= self.embed_dirs.len()
            {
                return Err(format!(
                    "its lookup log names --embed-dir {index} of {}",
                    self.embed_dirs.len()
                ));
            }
        }
        Ok(())
    }

    /// The directories the lookup `l` (one of [`LookupLog::lookups`])
    /// searched without finding its name, in order, up to its result
    /// (all it would search when it found nothing), as
    /// [`LookupLog::dirs`] names them; `.` is the working directory (an
    /// `#embed "..."` tries the name as written there, after the
    /// includer's directory). An absolute name searches none.
    #[must_use]
    pub fn tried(&self, l: &Lookup) -> Vec<&str> {
        if is_absolute(l) {
            return Vec::new();
        }
        let includer = l.includer.map(|i| self.dir(i));
        match l.form {
            LookupForm::Include | LookupForm::Next => {
                if l.found == Found::Includer {
                    return Vec::new();
                }
                let producer: HashMap<u32, usize> =
                    self.list.iter().enumerate().map(|(i, &d)| (d, i)).collect();
                let relative = l.form == LookupForm::Include && !l.angled;
                let first = includer.filter(|_| relative);
                let list = self.searched(l, &producer).filter_map(|j| self.list.get(j));
                first.into_iter().chain(list.map(|&d| self.dir(d))).collect()
            }
            LookupForm::Embed => {
                let mut out = Vec::new();
                if !l.angled {
                    if let Some(i) = includer {
                        if let Found::Embed { path } = &l.found
                            && *path == join_path(i, &l.name)
                        {
                            return out;
                        }
                        out.push(i);
                    }
                    if matches!(l.found, Found::Embed { .. }) {
                        return out;
                    }
                    out.push(".");
                }
                let end = match l.found {
                    Found::EmbedDir { index } => index as usize,
                    _ => self.embed_dirs.len(),
                };
                out.extend(self.embed_dirs.iter().take(end).map(|&d| self.dir(d)));
                out
            }
        }
    }

    fn dir(&self, i: u32) -> &str {
        self.dirs.get(i as usize).map_or("", String::as_str)
    }

    /// A recorded result with its directory named.
    fn result<'a>(&'a self, f: &'a Found) -> Res<'a> {
        match f {
            Found::Missing => Res::Missing,
            Found::Includer => Res::Includer,
            Found::Dir { dir, characteristic } => Res::Dir(self.dir(*dir), *characteristic),
            Found::Embed { path } => Res::Embed(path),
            Found::EmbedDir { index } => {
                Res::EmbedDir(self.embed_dirs.get(*index as usize).map_or("", |&d| self.dir(d)))
            }
            Found::Absolute => Res::Absolute,
        }
    }

    /// The path the result `r` of `l` names.
    fn path(&self, l: &Lookup, r: &Res<'_>) -> Option<String> {
        match r {
            Res::Missing => None,
            Res::Includer => Some(join_path(l.includer.map_or(".", |i| self.dir(i)), &l.name)),
            Res::Dir(d, _) | Res::EmbedDir(d) => Some(join_path(d, &l.name)),
            Res::Embed(path) => Some((*path).to_owned()),
            Res::Absolute => Some(l.name.clone()),
        }
    }

    /// `<name>`, `"name"`, `#include_next <name>`, `#embed "name"`.
    fn spelling(l: &Lookup) -> String {
        let name = if l.angled { format!("<{}>", l.name) } else { format!("\"{}\"", l.name) };
        match l.form {
            LookupForm::Include => name,
            LookupForm::Next => format!("#include_next {name}"),
            LookupForm::Embed => format!("#embed {name}"),
        }
    }

    /// Repeats every lookup, in order, over `list` (and `embed_dirs`, the
    /// consumer's `--embed-dir` list: [`LookupLog::embed_dir_names`] is
    /// the producer's) and `fs`, and names the first whose result
    /// differs from the recorded one (spec/pch.md "Lookup-tolerant search
    /// path", spec/pp.md "Lookup log"). `known(path)` says that a file is
    /// known to exist (the PCH record's file list, just checked), so it
    /// is not asked about. `trust` says which of the producer's misses
    /// are taken as they were ([`Trust`]), for `#embed` as for
    /// `#include`: a directory [`LookupLog::tried`] names is not asked
    /// again when trusted.
    ///
    /// # Errors
    /// The first lookup that finds something else, as `-v` names it.
    pub fn check(
        &self,
        list: &SearchList,
        embed_dirs: &[String],
        fs: &mut dyn HeaderFs,
        known: &dyn Fn(&str) -> bool,
        trust: Trust,
    ) -> Result<(), String> {
        let mut p = Prober::new(fs, known);
        // The position of each directory of `list`, by name.
        let index: HashMap<&str, usize> =
            list.dirs().iter().enumerate().map(|(i, d)| (d.path.as_str(), i)).collect();
        let trusted = |d: &str| match trust {
            Trust::ProducerMisses { same_cwd } => same_cwd || d.starts_with('/'),
            Trust::Nothing => false,
        };
        // The position of each directory of the producer's list, and its
        // position in `list` when a miss there can be trusted.
        let producer: HashMap<u32, usize> =
            self.list.iter().enumerate().map(|(i, &d)| (d, i)).collect();
        let to_consumer: Vec<Option<usize>> = self
            .list
            .iter()
            .map(|&d| Some(self.dir(d)).filter(|d| trusted(d)).and_then(|d| index.get(d).copied()))
            .collect();
        // Per directory of `list`: searched in vain by the producer's
        // current lookup; its listing, once read.
        let mut missed = vec![false; list.dirs().len()];
        let mut tops: Vec<Top> = vec![Top::Unread; list.dirs().len()];
        // The directories the producer's list does not have are read
        // first, together: each is listed when first reached anyway,
        // and a read waits on the file system.
        let new: Vec<usize> = (0..list.dirs().len())
            .filter(|c| {
                let d = list.dirs()[*c].path.as_str();
                !d.is_empty() && d != "." && !to_consumer.contains(&Some(*c))
            })
            .collect();
        if !self.lookups.is_empty() && new.len() > 1 {
            let paths: Vec<&str> = new.iter().map(|&c| list.dirs()[c].path.as_str()).collect();
            for (&c, names) in new.iter().zip(p.fs.list_dirs(&paths)) {
                tops[c] = Top::of(names);
            }
        }
        for l in &self.lookups {
            if is_absolute(l) {
                self.repeat_absolute(l, &mut p)?;
                continue;
            }
            let now = match l.form {
                LookupForm::Include | LookupForm::Next => {
                    let start = match (l.form, l.after) {
                        (LookupForm::Next, Some(a)) => {
                            let Some(&j) = index.get(self.dir(a)) else {
                                return Err(format!(
                                    "lookup of {} continues after '{}', which is not in the search list",
                                    Self::spelling(l),
                                    self.dir(a)
                                ));
                            };
                            j + 1
                        }
                        _ if l.angled => list.angled_start(),
                        _ => 0,
                    };
                    missed.fill(false);
                    for j in self.searched(l, &producer) {
                        if let Some(Some(c)) = to_consumer.get(j) {
                            missed[*c] = true;
                        }
                    }
                    let hit = match self.result(&l.found) {
                        Res::Dir(r, _) => index.get(r).copied(),
                        _ => None,
                    };
                    let relative = l.form == LookupForm::Include && !l.angled;
                    let includer = l.includer.map_or(".", |i| self.dir(i));
                    // The name's first component, as directory listings
                    // hold it.
                    let first = l.name.split('/').next().unwrap_or("");
                    let first =
                        (!first.is_empty() && first != "." && first != ".." && first.is_ascii())
                            .then(|| first.to_ascii_lowercase());
                    if relative
                        && !(l.found != Found::Includer && trusted(includer))
                        && p.has(includer, &l.name)
                    {
                        Res::Includer
                    } else {
                        let dirs = list.dirs();
                        (start.min(dirs.len())..dirs.len())
                            .find(|&c| {
                                let d = &dirs[c].path;
                                // The recorded directory's file is in
                                // the record's list: no listing needed.
                                !missed[c]
                                    && (hit == Some(c)
                                        || p.may_have(&mut tops[c], d, first.as_deref()))
                                    && p.has(d, &l.name)
                            })
                            .map_or(Res::Missing, |c| {
                                Res::Dir(&dirs[c].path, dirs[c].characteristic)
                            })
                    }
                }
                LookupForm::Embed => {
                    self.embed(l, embed_dirs, &mut p, &trusted)?;
                    continue;
                }
            };
            let was = self.result(&l.found);
            if now != was {
                return Err(self.difference(l, &was, &now));
            }
        }
        Ok(())
    }

    /// Repeats only the lookups of absolute names, in order, over `fs`
    /// (WP-2620): what [`LookupLog::check`] repeats of them, for a
    /// consumer that takes every other lookup as recorded (the PCH
    /// consumer with the producer's search list and working directory).
    /// An absolute name's result depends on the file system alone, so a
    /// miss is asked again whatever the trust: one `stat` each, none for
    /// a `known` file.
    ///
    /// # Errors
    /// The first lookup that finds something else, as `-v` names it.
    pub fn check_absolute(
        &self,
        fs: &mut dyn HeaderFs,
        known: &dyn Fn(&str) -> bool,
    ) -> Result<(), String> {
        let mut p = Prober::new(fs, known);
        self.lookups
            .iter()
            .filter(|l| is_absolute(l))
            .try_for_each(|l| self.repeat_absolute(l, &mut p))
    }

    /// Repeats the lookup `l` of an absolute name: opened as is (an
    /// `#include_next` that continues after a directory finds nothing
    /// and asks nothing), by `#embed` too.
    fn repeat_absolute(&self, l: &Lookup, p: &mut Prober<'_>) -> Result<(), String> {
        let now = match l.form {
            LookupForm::Include if p.file(&l.name) => Res::Absolute,
            LookupForm::Embed if p.file(&l.name) => Res::Embed(&l.name),
            LookupForm::Include | LookupForm::Embed | LookupForm::Next => Res::Missing,
        };
        let was = self.result(&l.found);
        if now == was { Ok(()) } else { Err(self.difference(l, &was, &now)) }
    }

    /// Repeats the `#embed` lookup `l` over `embed_dirs` (an embed does
    /// not use the search list): the includer's directory, the working
    /// directory, then `embed_dirs`, skipping a directory the producer
    /// searched in vain ([`LookupLog::tried`]) when `trusted`.
    fn embed(
        &self,
        l: &Lookup,
        embed_dirs: &[String],
        p: &mut Prober<'_>,
        trusted: &dyn Fn(&str) -> bool,
    ) -> Result<(), String> {
        let missed: HashSet<&str> = self.tried(l).into_iter().filter(|d| trusted(d)).collect();
        let mut tries: Vec<(&str, String)> = Vec::new();
        if !l.angled {
            if let Some(i) = l.includer {
                tries.push((self.dir(i), join_path(self.dir(i), &l.name)));
            }
            tries.push((".", l.name.clone()));
        }
        tries.extend(embed_dirs.iter().map(|d| (d.as_str(), join_path(d, &l.name))));
        let found = tries.into_iter().find(|(d, t)| !missed.contains(d) && p.file(t));
        let now = found.as_ref().map_or(Res::Missing, |(_, t)| Res::Embed(t));
        let was = self.result(&l.found);
        if self.path(l, &now) == self.path(l, &was) {
            Ok(())
        } else {
            Err(self.difference(l, &was, &now))
        }
    }

    /// The producer's `--embed-dir` list ([`LookupLog::embed_dirs`]) by
    /// name, in order.
    #[must_use]
    pub fn embed_dir_names(&self) -> Vec<&str> {
        self.embed_dirs.iter().map(|&d| self.dir(d)).collect()
    }

    /// The positions in the producer's list of the directories the
    /// lookup `l` searched without finding its name: from where it
    /// started to its result (all of its range when it found nothing).
    fn searched(&self, l: &Lookup, producer: &HashMap<u32, usize>) -> std::ops::Range<usize> {
        let start = match (l.form, l.after) {
            (LookupForm::Next, Some(a)) => match producer.get(&a) {
                Some(&j) => j + 1,
                None => return 0..0,
            },
            _ if l.angled => usize::try_from(self.angled_start).unwrap_or(usize::MAX),
            _ => 0,
        };
        let end = match &l.found {
            Found::Missing => self.list.len(),
            Found::Dir { dir, .. } => match producer.get(dir) {
                Some(&j) => j,
                None => return 0..0,
            },
            Found::Includer | Found::Embed { .. } | Found::EmbedDir { .. } | Found::Absolute => {
                return 0..0;
            }
        };
        start.min(end)..end
    }

    /// How the result `now` of `l` differs from the recorded `was`.
    fn difference(&self, l: &Lookup, was: &Res<'_>, now: &Res<'_>) -> String {
        let what = Self::spelling(l);
        let kind = |r: &Res<'_>| match r {
            Res::Dir(_, Characteristic::User) => "a user header",
            Res::Dir(_, Characteristic::System) => "a system header",
            Res::Dir(_, Characteristic::ExternCSystem) => "an extern \"C\" system header",
            _ => "a header relative to its includer",
        };
        match (self.path(l, was), self.path(l, now)) {
            (Some(w), Some(n)) if w == n => {
                format!("lookup of {what} now finds '{n}' as {} (was {})", kind(now), kind(was))
            }
            (Some(w), Some(n)) => format!("lookup of {what} now finds '{n}' (was '{w}')"),
            (None, Some(n)) => format!("lookup of {what} now finds '{n}' (was not found)"),
            (Some(w), None) => format!("lookup of {what} now finds nothing (was '{w}')"),
            (None, None) => format!("lookup of {what} differs"),
        }
    }
}

/// Which of a log's misses [`LookupLog::check`] takes as recorded.
///
/// A log is a statement about one view of the file system: the disk, or
/// the disk as a run's overlay changes it (spec/source.md "Overlay"). It
/// is replayed with [`Trust::ProducerMisses`] only against the view it
/// was recorded under; under another overlay, or under none after one,
/// with [`Trust::Nothing`] and a file system of the consumer's view
/// (`OverlayFs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    /// A directory the producer's lookup searched without finding the
    /// name (one of its list before the result, or the includer's
    /// directory) is not asked again, as the fast path does not ask: the
    /// file system is taken to be the producer's but for the files
    /// `known` lists. That holds for a relative directory only in the
    /// producer's working directory (`same_cwd`). The PCH consumer's
    /// replay.
    ProducerMisses {
        /// The consumer's working directory is the producer's.
        same_cwd: bool,
    },
    /// Every directory before a result is asked again (listings keep it
    /// cheap): for a view other than the producer's.
    Nothing,
}

/// A result with its directory named (the recorded one, or the
/// consumer's).
#[derive(Debug, PartialEq, Eq)]
enum Res<'a> {
    Missing,
    Includer,
    Dir(&'a str, Characteristic),
    Embed(&'a str),
    /// Found in this `--embed-dir`.
    EmbedDir(&'a str),
    Absolute,
}

/// Whether `l` looks up an absolute name, which is opened as is.
fn is_absolute(l: &Lookup) -> bool {
    l.name.starts_with('/')
}

/// A search directory's listing in [`LookupLog::check`].
#[derive(Clone, Debug)]
enum Top {
    /// Not read yet.
    Unread,
    /// It cannot be listed.
    Unlisted,
    /// Its entries, ASCII-lowercased.
    Entries(HashSet<String>),
}

impl Top {
    fn of(names: Option<Vec<String>>) -> Top {
        Listing::of(names).map_or(Top::Unlisted, Top::Entries)
    }
}

/// Existence queries for [`LookupLog::check`], each asked once, with
/// header search's directory listings ([`Listing`]).
struct Prober<'f> {
    fs: &'f mut dyn HeaderFs,
    known: &'f dyn Fn(&str) -> bool,
    files: HashMap<String, bool>,
    dirs: HashMap<String, bool>,
    listings: HashMap<String, Listing>,
}

impl<'f> Prober<'f> {
    fn new(fs: &'f mut dyn HeaderFs, known: &'f dyn Fn(&str) -> bool) -> Prober<'f> {
        Prober { fs, known, files: HashMap::new(), dirs: HashMap::new(), listings: HashMap::new() }
    }

    /// Whether the search directory `dir` may hold a name whose first
    /// component is `first` (lowercased; `None` when a listing cannot
    /// tell): `false` only when `dir`'s listing has no such entry. The
    /// listing is read into `top` once, the first time `dir` is
    /// reached, so each later miss there costs no query (`None` inside:
    /// it cannot be listed).
    fn may_have(&mut self, top: &mut Top, dir: &str, first: Option<&str>) -> bool {
        let Some(first) = first else { return true };
        if dir.is_empty() || dir == "." {
            return true;
        }
        if matches!(top, Top::Unread) {
            *top = Top::of(self.fs.list_dir(dir));
        }
        match top {
            Top::Entries(e) => e.contains(first),
            Top::Unread | Top::Unlisted => true,
        }
    }

    /// Whether the regular file `path` exists.
    fn file(&mut self, path: &str) -> bool {
        if let Some(&r) = self.files.get(path) {
            return r;
        }
        let r = (self.known)(path) || self.fs.is_file(path);
        self.files.insert(path.to_owned(), r);
        r
    }

    /// Whether `name` is a regular file in the directory `dir`, as
    /// header search's `open_in` finds it: each directory `name` passes
    /// through must exist, and a component missing from a listed
    /// directory is a miss without a query.
    fn has(&mut self, dir: &str, name: &str) -> bool {
        let path = join_path(dir, name);
        if let Some(&r) = self.files.get(&path) {
            return r;
        }
        if (self.known)(&path) {
            self.files.insert(path, true);
            return true;
        }
        let mut start = path.len() - name.len();
        loop {
            let end = path[start..].find('/').map(|i| start + i);
            let last = end.is_none();
            let end = end.unwrap_or(path.len());
            let (parent, leaf) = (&path[..start], &path[start..end]);
            if self.listed(parent, leaf) == Some(false) {
                self.files.insert(path, false);
                return false;
            }
            if last {
                let r = self.fs.is_file(&path);
                if !r {
                    self.note_miss(start, &path);
                }
                self.files.insert(path, r);
                return r;
            }
            if !leaf.is_empty() && !self.dir_exists(&path[..end]) {
                self.note_miss(start, &path);
                self.files.insert(path, false);
                return false;
            }
            start = end + 1;
        }
    }

    fn dir_exists(&mut self, path: &str) -> bool {
        if let Some(&r) = self.dirs.get(path) {
            return r;
        }
        let r = self.fs.is_dir(path);
        self.dirs.insert(path.to_owned(), r);
        r
    }

    fn listed(&self, parent: &str, leaf: &str) -> Option<bool> {
        if !listable(parent, leaf) {
            return None;
        }
        let entries = self.listings.get(parent)?.entries.as_ref()?;
        Some(entries.contains(&leaf.to_ascii_lowercase()))
    }

    fn note_miss(&mut self, start: usize, path: &str) {
        let parent = &path[..start];
        let leaf = path[start..].split('/').next().unwrap_or("");
        if !listable(parent, leaf) {
            return;
        }
        let listing = self.listings.entry(parent.to_owned()).or_default();
        if listing.entries.is_some() || listing.misses >= LIST_AFTER_MISSES {
            return;
        }
        listing.misses += 1;
        if listing.misses == LIST_AFTER_MISSES {
            let dir = if parent.len() > 1 { &parent[..parent.len() - 1] } else { parent };
            listing.entries = Listing::read(self.fs, dir);
        }
    }
}
