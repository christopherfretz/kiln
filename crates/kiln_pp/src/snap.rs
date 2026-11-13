//! The snapshot encoding of [`Preprocessor`] (spec/pp.md "Snapshots",
//! spec/snap.md "Configured roots").
//!
//! The preprocessor is a configured root: it borrows the source manager,
//! the interner and the diagnostics engine, and holds the consumer's
//! include resolver, `#if` evaluator, feature queries, callbacks and
//! options, so it is not `Snap`. [`Preprocessor::snap_put`] writes the
//! state of a quiescent preprocessor (the main file closed by
//! [`Preprocessor::end_main_file`]); [`Preprocessor::snap_restore`]
//! applies it to a fresh one the consumer built with
//! [`Preprocessor::new`] from its own options. Both name every field (an
//! exhaustive destructure), so a new field is a compile error here until
//! it is classified.
//!
//! | field | class | how |
//! |---|---|---|
//! | `sm`, `interner`, `diags` | configuration | the consumer's, restored before the preprocessor is built |
//! | `opts`, `lex_opts` | configuration | from the consumer's command line (the snapshot's configuration fingerprint covers them) |
//! | `resolver`, `evaluator`, `features`, `callbacks`, `comment_handler` | configuration | trait objects the consumer installs (the resolver's lookup caches refill: the restored source manager gives the same `FileId` for the same path) |
//! | `state_hooks` | configuration | the replay record's hooks (spec/pp.md §13); not carried |
//! | `macros` | state | every definition (builtins included); `Rc` sharing with `pushed` is kept |
//! | `pushed`, `poisoned`, `once_files`, `guards`, `entered` | state | as data |
//! | `scratch`, `embed_bufs` | state | scratch buffers by text (reused, so ids stay as the producer's) |
//! | `gcc_dirs` | state | the first lookup in a directory fixes its GCC spelling |
//! | `counter` | state | `__COUNTER__` |
//! | `include_limit_reached` | state | as data |
//! | `unit_first_hash`, `unit_pre_directive` | state | as data (P3868R1 bookkeeping of the main file) |
//! | `main_file`, `predef_file` | state | the producer's; the next main file replaces `main_file` ([`Preprocessor::enter_next_main_file`]) |
//! | `diag_pushes` | state | as data |
//! | `attr_macros`, `attr_live`, `attr_macro_gen` | state | as data |
//! | `modules` | configuration + state + at rest | the options are the consumer's; `builtin_file` is written; every other field must be empty ([`crate::modules`] `snap_unready`): a prefix that imports or declares a module is refused |
//! | `syms`, `dirs` | rebuilt | by the consumer's [`Preprocessor::new`] from the restored interner (the same symbols) |
//! | `date_time` | rebuilt | `__DATE__`/`__TIME__` of the consumer's compilation, on first use |
//! | `module_lines` | rebuilt | by the next main file's entry |
//! | `pool`, `arg_pool`, `paste_buf`, `dir_hash` | transient | recycled buffers and the last directive's `#`: reset |
//! | `frames`, `markers`, `pending`, `include_depth` | transient, at rest | no file open, nothing queued |
//! | `if_ndef`, `disable_expand`, `in_args`, `arg_preexpand`, `parsing_if`, `va_scope`, `in_directive`, `dir_eod`, `quiet_lex` | transient, at rest | no directive, argument or `#if` being read |
//! | `name_gate` | at rest | must be off (`-fkiln-bmi-image` is not supported with snapshots) |

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use kiln_lex::{Symbol, TokKind};
use kiln_snap::{Desc, Reader, Schema, Snap, SnapError, Writer};
use kiln_source::{FileId, Loc, Span};

use crate::Preprocessor;
use crate::macros::{AttrMacro, MacroDef, MacroTable, SymMap};
use crate::preprocessor::Frame;

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

/// The state a snapshot carries, decoded and validated before any of it
/// is applied.
struct Decoded {
    macros: MacroTable,
    pushed: SymMap<Vec<Option<Rc<MacroDef>>>>,
    poisoned: SymMap<Span>,
    once_files: HashSet<FileId>,
    guards: HashMap<FileId, Symbol>,
    entered: HashSet<FileId>,
    scratch: HashMap<Box<str>, FileId>,
    embed_bufs: HashSet<FileId>,
    gcc_dirs: HashMap<Box<str>, bool>,
    counter: u32,
    include_limit_reached: bool,
    unit_first_hash: Option<Loc>,
    unit_pre_directive: bool,
    main_file: Option<FileId>,
    predef_file: Option<FileId>,
    diag_pushes: u32,
    attr_macros: Vec<AttrMacro>,
    attr_live: SymMap<usize>,
    attr_macro_gen: u32,
    builtin_file: Option<FileId>,
}

impl Preprocessor<'_, '_> {
    /// The snapshot point (spec/pp.md "Snapshots"): once the main file
    /// has ended ([`Self::next`] returned `Eof`), closes it as the end of
    /// an included file would (its include guard is recorded), so that no
    /// file is open. Then [`Self::snap_put`] can write the state, and
    /// [`Self::enter_next_main_file`] goes on with another main file.
    ///
    /// # Errors
    /// The main file is not the only open file, or has not ended.
    pub fn end_main_file(&mut self) -> Result<(), String> {
        let [Frame::File(f)] = self.frames.as_slice() else {
            return Err(format!(
                "Preprocessor.frames: {} open, not the main file alone",
                self.frames.len()
            ));
        };
        if Some(f.file) != self.main_file
            || !f.back.is_empty()
            || !f.conds.is_empty()
            || (f.lx.offset() as usize) < self.sm.text(f.file).len()
            || !self.markers.is_empty()
            || self.pending.is_some()
        {
            return Err("Preprocessor.frames: the main file has not ended".to_owned());
        }
        let Some(Frame::File(f)) = self.frames.pop() else { unreachable!("one file frame") };
        if f.traced {
            kiln_source::trace::source_end();
        }
        if let Some(m) = f.mi.controlling_macro() {
            let main_dir = crate::search::parent_dir(self.sm.file_name(f.file));
            if let Some(h) = self.resolver.suggest_include(self.sm, f.file, Some(main_dir)) {
                self.sm.mark_include_guarded(f.file, h.into());
            }
            self.guards.insert(f.file, m);
        }
        Ok(())
    }

    /// Enters `file` as the main file of a preprocessor with no open
    /// file: after [`Self::end_main_file`], or after
    /// [`Self::snap_restore`]. Unlike [`Self::enter_main_file`], the
    /// predefines are not read again: they are part of the state. The
    /// `#pragma clang diagnostic` state in effect at the end of the prefix
    /// continues at the start of `file`, as it continues in the includer
    /// after the end of an `-include`d header.
    ///
    /// # Errors
    /// A file is open.
    pub fn enter_next_main_file(&mut self, file: FileId) -> Result<(), String> {
        if !self.frames.is_empty() {
            return Err(format!("Preprocessor.frames: {} open", self.frames.len()));
        }
        self.main_file = Some(file);
        self.push_file(file, None, None, None, false, false);
        self.diags.pragma_file_exit(self.sm.loc(file, 0));
        if self.modules.is_some() {
            self.module_lines = crate::modules::has_module_lines(self.sm.text(file));
        }
        Ok(())
    }

    /// After [`Self::enter_next_main_file`]: enters `text`, the consumer's
    /// own predefines buffer reduced to the `-include`s that follow a
    /// restored precompiled header (WP-1383), as a new `<built-in>`
    /// buffer on top of the main file, as [`Self::enter_main_file`] enters
    /// the full predefines. It becomes the predefines buffer (its
    /// `#include`s are found from the working directory).
    ///
    /// # Errors
    /// The main file is not the only open file, or location space is
    /// exhausted.
    pub fn enter_builtin_includes(&mut self, text: String) -> Result<(), String> {
        if self.frames.len() != 1 {
            return Err(format!("Preprocessor.frames: {} open", self.frames.len()));
        }
        let b = self.sm.add_virtual("<built-in>", text).map_err(|e| e.to_string())?;
        self.push_file(b, None, None, None, false, false);
        self.predef_file = Some(b);
        Ok(())
    }

    /// Whether the preprocessor is at rest for a snapshot (spec/pp.md
    /// "Snapshots"): no file open, nothing queued or read ahead, no
    /// directive, macro argument or `#if` being read, no module state but
    /// the predefines buffer. The error names the first field that is not
    /// at rest (the innermost frame first).
    ///
    /// # Errors
    /// The field that is not at rest, and why.
    pub fn snap_ready(&self) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("Preprocessor.pending: a token is queued behind a marker".to_owned());
        }
        if !self.markers.is_empty() {
            return Err(format!("Preprocessor.markers: {} queued", self.markers.len()));
        }
        for (i, fr) in self.frames.iter().enumerate().rev() {
            match fr {
                Frame::File(f) if !f.back.is_empty() => {
                    return Err(format!(
                        "Preprocessor.frames[{i}].back: {} token(s) lexed ahead in `{}`",
                        f.back.len(),
                        self.sm.file_name(f.file)
                    ));
                }
                Frame::File(f) if !f.conds.is_empty() => {
                    return Err(format!(
                        "Preprocessor.frames[{i}].conds: inside {} conditional block(s) of `{}`",
                        f.conds.len(),
                        self.sm.file_name(f.file)
                    ));
                }
                Frame::Macro(_) => {
                    return Err(format!("Preprocessor.frames[{i}]: a macro expansion is open"));
                }
                Frame::File(_) => {}
            }
        }
        if let Some(Frame::File(f)) = self.frames.last() {
            let what = if Some(f.file) == self.main_file && self.frames.len() == 1 {
                "the main file is open (end it with end_main_file)"
            } else {
                "inside an included file"
            };
            return Err(format!(
                "Preprocessor.frames: {} open, {what}: `{}`",
                self.frames.len(),
                self.sm.file_name(f.file)
            ));
        }
        let busy = [
            ("include_depth", self.include_depth != 0),
            ("if_ndef", self.if_ndef.is_some()),
            ("disable_expand", self.disable_expand),
            ("in_args", self.in_args.is_some()),
            ("arg_preexpand", self.arg_preexpand != 0),
            ("parsing_if", self.parsing_if),
            ("va_scope", self.va_scope),
            ("in_directive", self.in_directive),
            ("dir_eod", self.dir_eod.is_some()),
            ("quiet_lex", self.quiet_lex),
            ("name_gate", self.name_gate.is_some()),
        ];
        if let Some((field, _)) = busy.iter().find(|b| b.1) {
            return Err(format!("Preprocessor.{field}: not at rest"));
        }
        if let Some(why) = self.modules.as_ref().and_then(|m| m.snap_unready()) {
            return Err(format!("Preprocessor.modules.{why}"));
        }
        let defs = self.macros.map.values().chain(self.pushed.values().flatten().flatten());
        if let Some(d) = defs.into_iter().find(|d| !d.enabled.get()) {
            let name = self.interner.as_str(d.name);
            return Err(format!("Preprocessor.macros: `{name}` is being expanded"));
        }
        Ok(())
    }

    /// Writes the preprocessor's state (spec/snap.md "Configured
    /// roots"); the writer's mode decides the map order
    /// ([`Writer::canonical`]). `Rc<MacroDef>` sharing between the macro
    /// table and the `push_macro` stacks is kept.
    ///
    /// # Errors
    /// [`Self::snap_ready`]'s: nothing is written then.
    pub fn snap_put(&self, w: &mut Writer) -> Result<(), String> {
        self.snap_ready()?;
        let Preprocessor {
            sm: _,
            diags: _,
            interner: _,
            opts: _,
            lex_opts: _,
            macros,
            frames: _,
            resolver: _,
            evaluator: _,
            features: _,
            callbacks: _,
            markers: _,
            pending: _,
            scratch,
            pool: _,
            arg_pool: _,
            syms: _,
            dirs: _,
            counter,
            once_files,
            guards,
            if_ndef: _,
            poisoned,
            pushed,
            entered,
            gcc_dirs,
            include_depth: _,
            include_limit_reached,
            unit_first_hash,
            unit_pre_directive,
            main_file,
            disable_expand: _,
            in_args: _,
            arg_preexpand: _,
            parsing_if: _,
            va_scope: _,
            date_time: _,
            paste_buf: _,
            in_directive: _,
            dir_eod: _,
            quiet_lex: _,
            dir_hash: _,
            diag_pushes,
            embed_bufs,
            attr_macros,
            attr_live,
            attr_macro_gen,
            comment_handler: _,
            modules,
            module_lines: _,
            name_gate: _,
            predef_file,
            state_hooks: _,
        } = self;
        macros.put(w);
        pushed.put(w);
        poisoned.put(w);
        once_files.put(w);
        guards.put(w);
        entered.put(w);
        scratch.put(w);
        embed_bufs.put(w);
        gcc_dirs.put(w);
        counter.put(w);
        include_limit_reached.put(w);
        unit_first_hash.put(w);
        unit_pre_directive.put(w);
        main_file.put(w);
        predef_file.put(w);
        diag_pushes.put(w);
        attr_macros.put(w);
        attr_live.put(w);
        attr_macro_gen.put(w);
        modules.as_ref().and_then(|m| m.builtin_file).put(w);
        Ok(())
    }

    /// Applies a snapshot written by [`Self::snap_put`] to this
    /// preprocessor, which the consumer has just built with
    /// [`Self::new`] over the restored source manager, interner and
    /// diagnostics engine, and configured (resolver, evaluator, feature
    /// queries, callbacks). It must be fresh: no main file entered. Every
    /// id is the producer's; the next step is
    /// [`Self::enter_next_main_file`]. On an error the preprocessor is
    /// unchanged.
    ///
    /// # Errors
    /// A corrupt snapshot ([`SnapError`], never a panic), or a
    /// preprocessor that is not fresh (`ErrorKind::Invalid`).
    pub fn snap_restore(&mut self, r: &mut Reader<'_>) -> Result<(), SnapError> {
        if self.main_file.is_some() || self.predef_file.is_some() || self.snap_ready().is_err() {
            return Err(r.invalid::<Self>("restore into a used preprocessor"));
        }
        let d = Decoded {
            macros: at("Preprocessor.macros", r.get())?,
            pushed: at("Preprocessor.pushed", r.get())?,
            poisoned: at("Preprocessor.poisoned", r.get())?,
            once_files: at("Preprocessor.once_files", r.get())?,
            guards: at("Preprocessor.guards", r.get())?,
            entered: at("Preprocessor.entered", r.get())?,
            scratch: at("Preprocessor.scratch", r.get())?,
            embed_bufs: at("Preprocessor.embed_bufs", r.get())?,
            gcc_dirs: at("Preprocessor.gcc_dirs", r.get())?,
            counter: at("Preprocessor.counter", r.get())?,
            include_limit_reached: at("Preprocessor.include_limit_reached", r.get())?,
            unit_first_hash: at("Preprocessor.unit_first_hash", r.get())?,
            unit_pre_directive: at("Preprocessor.unit_pre_directive", r.get())?,
            main_file: at("Preprocessor.main_file", r.get())?,
            predef_file: at("Preprocessor.predef_file", r.get())?,
            diag_pushes: at("Preprocessor.diag_pushes", r.get())?,
            attr_macros: at("Preprocessor.attr_macros", r.get())?,
            attr_live: at("Preprocessor.attr_live", r.get())?,
            attr_macro_gen: at("Preprocessor.attr_macro_gen", r.get())?,
            builtin_file: at("Preprocessor.modules.builtin_file", r.get())?,
        };
        if let Err(why) = self.check_restored(&d) {
            return Err(r.invalid::<Self>(why));
        }
        let Decoded {
            macros,
            pushed,
            poisoned,
            once_files,
            guards,
            entered,
            scratch,
            embed_bufs,
            gcc_dirs,
            counter,
            include_limit_reached,
            unit_first_hash,
            unit_pre_directive,
            main_file,
            predef_file,
            diag_pushes,
            attr_macros,
            attr_live,
            attr_macro_gen,
            builtin_file,
        } = d;
        self.macros = macros;
        self.pushed = pushed;
        self.poisoned = poisoned;
        self.once_files = once_files;
        self.guards = guards;
        self.entered = entered;
        self.scratch = scratch;
        self.embed_bufs = embed_bufs;
        self.gcc_dirs = gcc_dirs;
        self.counter = counter;
        self.include_limit_reached = include_limit_reached;
        self.unit_first_hash = unit_first_hash;
        self.unit_pre_directive = unit_pre_directive;
        self.main_file = main_file;
        self.predef_file = predef_file;
        self.diag_pushes = diag_pushes;
        self.attr_macros = attr_macros;
        self.attr_live = attr_live;
        self.attr_macro_gen = attr_macro_gen;
        if let Some(m) = &mut self.modules {
            m.builtin_file = builtin_file;
        }
        Ok(())
    }

    /// The invariants the preprocessor relies on without a check, for a
    /// decoded state: every symbol interned, every file id allocated,
    /// every macro definition consistent and not being expanded, every
    /// live attribute macro indexing an entry still in effect.
    fn check_restored(&self, d: &Decoded) -> Result<(), &'static str> {
        let nsym = self.interner.len();
        let sym = |s: &Symbol| (s.index() as usize) < nsym;
        let nfile = self.sm.file_count();
        let file = |f: &FileId| f.index() < nfile;
        let def_ok = |k: Option<&Symbol>, m: &MacroDef| {
            k.is_none_or(|k| *k == m.name)
                && sym(&m.name)
                && m.params.iter().all(sym)
                && m.rel.len() == m.body.len()
                && m.body.iter().all(|t| match t.tok.kind {
                    TokKind::Identifier(s) => sym(&s),
                    _ => true,
                })
                && m.enabled.get()
                && (m.function_like || m.params.is_empty())
                && (!m.variadic || (m.function_like && !m.params.is_empty()))
                && (!m.named_variadic || m.variadic)
                && (m.body.is_empty() || self.body_spelled(m))
        };
        if !d.macros.map.iter().all(|(k, m)| def_ok(Some(k), m)) {
            return Err("a macro definition is inconsistent");
        }
        let pushed_ok =
            d.pushed.iter().all(|(k, v)| sym(k) && v.iter().flatten().all(|m| def_ok(Some(k), m)));
        if !pushed_ok {
            return Err("a push_macro entry is inconsistent");
        }
        if !d.poisoned.keys().all(sym)
            || !d.guards.values().all(sym)
            || !d.attr_live.keys().all(sym)
        {
            return Err("a symbol that is not interned");
        }
        let files =
            d.once_files.iter().chain(&d.entered).chain(&d.embed_bufs).chain(d.guards.keys());
        let opt_files = [d.main_file, d.predef_file, d.builtin_file];
        if !files.into_iter().chain(d.scratch.values()).chain(opt_files.iter().flatten()).all(file)
        {
            return Err("a file id that is not allocated");
        }
        if !d.attr_live.values().all(|&i| d.attr_macros.get(i).is_some_and(|a| !a.end.is_valid())) {
            return Err("a live attribute macro out of range");
        }
        Ok(())
    }

    /// A replacement list's locations, as the expansion code uses them
    /// without a check: the definition range is one allocated entry, and
    /// each token is spelled at its offset `rel` in it.
    fn body_spelled(&self, m: &MacroDef) -> bool {
        let start = m.def_start;
        let at = |n: u32| start.to_raw().checked_add(n).map(Loc::from_raw);
        at(m.def_len).is_some_and(|end| self.sm.same_entry_distance(start, end) == Some(m.def_len))
            && m.body.iter().zip(&m.rel).all(|(t, &(lo, hi))| {
                lo <= hi
                    && hi <= m.def_len
                    && Some(t.tok.span.lo) == at(lo)
                    && Some(t.tok.span.hi) == at(hi)
            })
    }

    /// Registers the snapshot's descriptor (the fields
    /// [`Self::snap_put`] writes) and those of the types it encodes.
    pub fn snap_schema(s: &mut Schema) {
        let d = Desc::new("configured root")
            .field::<MacroTable>("macros")
            .field::<SymMap<Vec<Option<Rc<MacroDef>>>>>("pushed")
            .field::<SymMap<Span>>("poisoned")
            .field::<HashSet<FileId>>("once_files")
            .field::<HashMap<FileId, Symbol>>("guards")
            .field::<HashSet<FileId>>("entered")
            .field::<HashMap<Box<str>, FileId>>("scratch")
            .field::<HashSet<FileId>>("embed_bufs")
            .field::<HashMap<Box<str>, bool>>("gcc_dirs")
            .field::<u32>("counter")
            .field::<bool>("include_limit_reached")
            .field::<Option<Loc>>("unit_first_hash")
            .field::<bool>("unit_pre_directive")
            .field::<Option<FileId>>("main_file")
            .field::<Option<FileId>>("predef_file")
            .field::<u32>("diag_pushes")
            .field::<Vec<AttrMacro>>("attr_macros")
            .field::<SymMap<usize>>("attr_live")
            .field::<u32>("attr_macro_gen")
            .field::<Option<FileId>>("modules.builtin_file");
        if s.register::<Preprocessor<'static, 'static>>(d) {
            MacroTable::schema(s);
            SymMap::<Vec<Option<Rc<MacroDef>>>>::schema(s);
            SymMap::<Span>::schema(s);
            HashSet::<FileId>::schema(s);
            HashMap::<FileId, Symbol>::schema(s);
            HashMap::<Box<str>, FileId>::schema(s);
            HashMap::<Box<str>, bool>::schema(s);
            u32::schema(s);
            bool::schema(s);
            Option::<Loc>::schema(s);
            Option::<FileId>::schema(s);
            Vec::<AttrMacro>::schema(s);
            SymMap::<usize>::schema(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
    use kiln_lex::{Interner, Std};
    use kiln_snap::{Reader, Writer};
    use kiln_source::SourceManager;

    use crate::{MemoryResolver, PpOptions, Preprocessor};

    /// [cpp.pragma] `push_macro`: the stack holds the very definition the
    /// table had (`Rc::ptr_eq`, which redefinition checks and the
    /// preprocessor-state record rely on), before and after a restore.
    #[test]
    fn push_macro_stack_shares_with_the_table_after_a_restore() {
        let src = "#define M 1\n#define N 2\n#pragma push_macro(\"M\")\n#pragma push_macro(\"M\")\n#pragma push_macro(\"N\")\n#undef N\nM\n";
        let (sm, interner) = (SourceManager::new(), Interner::new());
        let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
        let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
        pp.set_include_resolver(Box::new(MemoryResolver::new()));
        pp.enter_main_file(main);
        while !pp.next().is_eof() {}
        pp.end_main_file().unwrap();
        let mut w = Writer::new();
        pp.snap_put(&mut w).unwrap();
        let bytes = w.into_bytes();

        let m = interner.intern("M");
        let n = interner.intern("N");
        let shares = |pp: &Preprocessor<'_, '_>| {
            let def = pp.macros.get_rc(m).unwrap();
            let stack = &pp.pushed[&m];
            assert_eq!(stack.len(), 2);
            assert!(stack.iter().all(|d| Rc::ptr_eq(d.as_ref().unwrap(), def)));
            // `N` was pushed, then undefined: the stack alone holds it.
            assert!(pp.macros.get_rc(n).is_none());
            assert_eq!(Rc::strong_count(pp.pushed[&n][0].as_ref().unwrap()), 1);
        };
        shares(&pp);
        let mut fresh = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
        let mut r = Reader::new(&bytes);
        fresh.snap_restore(&mut r).unwrap();
        r.finish().unwrap();
        shares(&fresh);
    }
}
