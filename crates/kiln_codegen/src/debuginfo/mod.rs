//! DWARF debug information (spec/codegen.md "Debug information",
//! WP-814): clang's `CGDebugInfo`, driven by [`DebugOptions`].

/// clang's `codegenoptions::DebugInfoKind`, in increasing order of detail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugInfoKind {
    /// No debug information (`-g0`, the default).
    #[default]
    None,
    /// `-gline-directives-only`: `.loc` directives, no line table CU.
    LineDirectivesOnly,
    /// `-gline-tables-only` / `-g1` / `-gmlt`: line tables only.
    LineTablesOnly,
    /// `-flimit-debug-info` without constructor homing.
    Limited,
    /// `-g` (clang's default): limited, with class definitions homed to
    /// the TU that defines a non-trivial constructor.
    Constructor,
    /// `-fstandalone-debug`: every used type in full.
    Full,
    /// `-fno-eliminate-unused-debug-types`: also unused types.
    UnusedTypes,
}

/// The debug information options of a compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugOptions {
    /// How much to emit.
    pub kind: DebugInfoKind,
    /// The `"Dwarf Version"` module flag (`-gdwarf-N`, 5 by default);
    /// clang emits it whenever a version was given, even at `-g0`.
    pub dwarf_version: Option<u32>,
    /// Column numbers in locations (`-gno-column-info` clears it).
    pub column_info: bool,
    /// `-fdebug-prefix-map=` / `-ffile-prefix-map=` pairs, in command
    /// line order (clang applies the last matching one first).
    pub prefix_map: Vec<(String, String)>,
    /// The compilation directory (`-fdebug-compilation-dir=`, else the
    /// working directory).
    pub comp_dir: String,
    /// `-gsplit-dwarf`: the `.dwo` file named by the CU's `splitDebugFilename`.
    pub split_dwarf_file: Option<String>,
    /// The main source file as named on the command line (the compile
    /// unit's file; clang `-main-file-name` joined to its directory).
    pub main_file: String,
    /// The main source buffer (its MD5 is the compile unit file's checksum).
    pub main_id: Option<kiln_source::FileId>,
    /// The `DW_LANG_*` of the compile unit.
    pub language: &'static str,
}

impl Default for DebugOptions {
    fn default() -> Self {
        DebugOptions {
            kind: DebugInfoKind::None,
            dwarf_version: None,
            column_info: true,
            prefix_map: Vec::new(),
            comp_dir: String::new(),
            split_dwarf_file: None,
            main_file: String::new(),
            main_id: None,
            language: "DW_LANG_C_plus_plus_14",
        }
    }
}

impl DebugOptions {
    /// Whether a compile unit is emitted (any `-g` but `-g0`).
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.kind > DebugInfoKind::None
    }

    /// clang `hasReducedDebugInfo`: variables and types, not only lines.
    #[must_use]
    pub fn reduced(&self) -> bool {
        self.kind >= DebugInfoKind::Limited
    }

    /// The `DW_LANG_*` for a C++ standard (clang `CreateCompileUnit`;
    /// DWARF 5's newer C++ codes are not used before clang 20's
    /// `-gstrict-dwarf` rules).
    #[must_use]
    pub fn language_for(cplusplus: bool, at_least_cxx11: bool, at_least_cxx14: bool) -> &'static str {
        match (cplusplus, at_least_cxx11, at_least_cxx14) {
            (false, ..) => "DW_LANG_C11",
            (true, _, true) => "DW_LANG_C_plus_plus_14",
            (true, true, false) => "DW_LANG_C_plus_plus_11",
            (true, false, false) => "DW_LANG_C_plus_plus",
        }
    }

    /// clang `remapDIPath`: the first matching prefix of the reversed map.
    #[must_use]
    pub fn remap(&self, path: &str) -> String {
        for (old, new) in self.prefix_map.iter().rev() {
            if let Some(rest) = path.strip_prefix(old.as_str()) {
                return format!("{new}{rest}");
            }
        }
        path.to_owned()
    }
}

mod md5;

use kiln_ast::{DeclId, QualType};
use kiln_llvm::{MdField, MdKindId, MetadataId};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{Loc, SourceManager};

use crate::module::Cg;

/// A specialized node's fields, in print order (`opt` reorders them).
#[derive(Default)]
pub(crate) struct Fields(pub Vec<(Box<str>, MdField)>);

impl Fields {
    pub fn s(mut self, k: &str, v: &str) -> Self {
        self.0.push((k.into(), MdField::Str(v.into())));
        self
    }
    pub fn u(mut self, k: &str, v: u64) -> Self {
        self.0.push((k.into(), MdField::UInt(v)));
        self
    }
    pub fn i(mut self, k: &str, v: i64) -> Self {
        self.0.push((k.into(), MdField::Int(v)));
        self
    }
    pub fn n(mut self, k: &str, v: Option<MetadataId>) -> Self {
        self.0.push((k.into(), MdField::Node(v)));
        self
    }
    pub fn raw(mut self, k: &str, v: &str) -> Self {
        self.0.push((k.into(), MdField::Raw(v.into())));
        self
    }
    pub fn b(mut self, k: &str, v: bool) -> Self {
        self.0.push((k.into(), MdField::Bool(v)));
        self
    }
    /// `flags:` when any are set (`DIFlagA | DIFlagB`).
    pub fn flags(self, k: &str, f: &[&str]) -> Self {
        if f.is_empty() { self } else { self.raw(k, &f.join(" | ")) }
    }
}

/// The module's debug information state (clang `CGDebugInfo`); present
/// only when [`DebugOptions::enabled`].
pub(crate) struct Di<'a> {
    pub sm: &'a SourceManager,
    /// The `dbg` metadata kind.
    pub dbg: MdKindId,
    pub cu: MetadataId,
    /// The compile unit's file.
    pub cu_file: MetadataId,
    /// `!DIExpression()`.
    pub empty_expr: MetadataId,
    /// `!{}`.
    pub empty: MetadataId,
    /// Files by presumed name.
    files: FxHashMap<Box<str>, MetadataId>,
    /// `DILocation`s by (line, column, scope).
    locs: FxHashMap<(u32, u32, MetadataId), MetadataId>,
    /// Types (as written, after clang's `UnwrapTypeForDebugInfo`).
    pub types: FxHashMap<QualType, Option<MetadataId>>,
    /// Class placeholders by definition, completed at the end of the
    /// unit as a definition or a declaration (clang's type homing).
    pub classes: FxHashMap<DeclId, MetadataId>,
    pub class_order: Vec<DeclId>,
    /// Classes whose definition is already being built (cycle guard).
    pub completed: FxHashSet<DeclId>,
    /// Classes a constructor of which was emitted (constructor homing).
    pub ctor_emitted: FxHashSet<DeclId>,
    /// Member function declarations by first declaration.
    pub decl_sps: FxHashMap<DeclId, MetadataId>,
    /// Namespaces by first declaration.
    pub namespaces: FxHashMap<DeclId, MetadataId>,
    /// Static data member declarations by first declaration.
    pub static_members: FxHashMap<DeclId, MetadataId>,
    pub enums: Vec<MetadataId>,
    pub retained: Vec<MetadataId>,
    pub globals: Vec<MetadataId>,
    pub imported: Vec<MetadataId>,
}

impl<'a> Cg<'a> {
    /// Starts debug information for the unit (clang `CGDebugInfo`'s
    /// constructor and `CreateCompileUnit`): the compile unit is the
    /// first `!llvm.dbg.cu`.
    pub(crate) fn start_debug_info(&mut self, sm: &'a SourceManager) {
        let o = &self.opts.debug;
        if !o.enabled() {
            return;
        }
        let mut name = if o.main_file.is_empty() { "<stdin>" } else { o.main_file.as_str() };
        while let Some(r) = name.strip_prefix("./") {
            name = r;
        }
        let checksum = o.main_id.map(|f| md5::hex(sm.text(f).as_bytes()));
        let cu_file = self.di_create_file(name, checksum.as_deref());
        let kind = match o.kind {
            DebugInfoKind::LineDirectivesOnly => "DebugDirectivesOnly",
            DebugInfoKind::LineTablesOnly => "LineTablesOnly",
            _ => "FullDebug",
        };
        let mut f = Fields::default()
            .raw("language", o.language)
            .n("file", Some(cu_file))
            .s("producer", &self.opts.ident)
            .b("isOptimized", self.opts.opt_level > 0)
            .u("runtimeVersion", 0);
        if let Some(dwo) = &o.split_dwarf_file {
            f = f.s("splitDebugFilename", dwo);
        }
        f = f.raw("emissionKind", kind).b("splitDebugInlining", false).raw("nameTableKind", "None");
        let cu = self.m.md_specialized("DICompileUnit", true, f.0);
        self.m.add_named_metadata("llvm.dbg.cu", &[cu]);
        let dbg = self.m.md_kind("dbg");
        let empty_expr = self.m.md_specialized("DIExpression", false, Vec::new());
        let empty = self.m.md_tuple(&[]);
        self.di = Some(Box::new(Di {
            sm,
            dbg,
            cu,
            cu_file,
            empty_expr,
            empty,
            files: FxHashMap::default(),
            locs: FxHashMap::default(),
            types: FxHashMap::default(),
            classes: FxHashMap::default(),
            class_order: Vec::new(),
            completed: FxHashSet::default(),
            ctor_emitted: FxHashSet::default(),
            decl_sps: FxHashMap::default(),
            namespaces: FxHashMap::default(),
            static_members: FxHashMap::default(),
            enums: Vec::new(),
            retained: Vec::new(),
            globals: Vec::new(),
            imported: Vec::new(),
        }));
    }

    /// clang `CGDebugInfo::createFile`: the remapped name split against
    /// the remapped compilation directory.
    fn di_create_file(&mut self, name: &str, checksum: Option<&str>) -> MetadataId {
        let o = &self.opts.debug;
        let file = o.remap(name);
        let cur = o.remap(&o.comp_dir);
        let (dir, file) = split_against(&file, &cur, name.starts_with('/'));
        let mut f = Fields::default().s("filename", &file).s("directory", &dir);
        if let Some(c) = checksum.filter(|_| o.dwarf_version.unwrap_or(5) >= 5) {
            f = f.raw("checksumkind", "CSK_MD5").s("checksum", c);
        }
        self.m.md_specialized("DIFile", false, f.0)
    }

    /// clang `getOrCreateFile`: the file of `loc`'s presumed position
    /// (the compile unit's file for an invalid location).
    pub(crate) fn di_file(&mut self, loc: Loc) -> MetadataId {
        let Some(di) = self.di.as_deref() else { unreachable_md() };
        let sm = di.sm;
        let Some(p) = sm.presumed_line_col(loc) else { return di.cu_file };
        let name = sm.presumed_file_name(&p, loc);
        if let Some(&f) = di.files.get(name.as_str()) {
            return f;
        }
        // A `#line` name has no buffer to checksum.
        let checksum = (p.file_name == sm.file_name(p.file)).then(|| md5::hex(sm.text(p.file).as_bytes()));
        let f = self.di_create_file(&name, checksum.as_deref());
        if let Some(di) = self.di.as_deref_mut() {
            di.files.insert(name.into(), f);
        }
        f
    }

    /// clang `getLineNumber` (0 for an invalid location).
    pub(crate) fn di_line(&self, loc: Loc) -> u32 {
        self.di.as_deref().and_then(|d| d.sm.presumed_line_col(loc)).map_or(0, |p| p.line)
    }

    /// A `DILocation` of `loc` in `scope`; `None` when `loc` is invalid.
    pub(crate) fn di_location(&mut self, loc: Loc, scope: MetadataId) -> Option<MetadataId> {
        let di = self.di.as_deref()?;
        let p = di.sm.presumed_line_col(loc)?;
        let col = if self.opts.debug.column_info { p.col } else { 0 };
        Some(self.di_location_at(p.line, col, scope))
    }

    /// The `DILocation` `line:col` in `scope`.
    pub(crate) fn di_location_at(&mut self, line: u32, col: u32, scope: MetadataId) -> MetadataId {
        let key = (line, col, scope);
        if let Some(&l) = self.di.as_deref().and_then(|d| d.locs.get(&key)) {
            return l;
        }
        let mut f = Fields::default().u("line", u64::from(line));
        if col != 0 {
            f = f.u("column", u64::from(col));
        }
        let l = self.m.md_specialized("DILocation", false, f.n("scope", Some(scope)).0);
        if let Some(d) = self.di.as_deref_mut() {
            d.locs.insert(key, l);
        }
        l
    }

    // ---- function locations (clang `CodeGenFunction` + `CGDebugInfo`) ---

    /// Whether the current function has debug locations.
    #[inline]
    pub(crate) fn dbg_on(&self) -> bool {
        self.f.fir.dbg.is_some()
    }

    /// clang `CGDebugInfo::EmitLocation`: the builder location becomes
    /// `loc` in the innermost scope; an invalid location changes nothing.
    pub(crate) fn dbg_emit_location(&mut self, loc: Loc) {
        let Some(d) = self.f.fir.dbg.as_deref() else { return };
        let scope = d.scope();
        if let Some(l) = self.di_location(loc, scope) {
            self.f.fir.set_dbg_loc(Some(l));
        }
    }

    /// clang `EmitStopPoint`.
    pub(crate) fn dbg_stop_point(&mut self, loc: Loc) {
        let Some(d) = self.f.fir.dbg.as_deref() else { return };
        let scope = d.scope();
        if let Some(l) = self.di_location(loc, scope) {
            self.f.fir.set_dbg_loc(Some(l));
            if let Some(d) = self.f.fir.dbg.as_deref_mut() {
                d.last_stop = Some(l);
            }
        }
    }

    /// clang `ApplyDebugLocation(CGF, Loc)`'s constructor: returns the
    /// location to restore (`None` when the function has none).
    #[inline]
    pub(crate) fn dbg_apply(&mut self, loc: Loc) -> Option<Option<MetadataId>> {
        let d = self.f.fir.dbg.as_deref()?;
        Some(self.dbg_apply_slow(loc, d.loc, d.scope()))
    }

    fn dbg_apply_slow(&mut self, loc: Loc, orig: Option<MetadataId>, scope: MetadataId) -> Option<MetadataId> {
        if orig.is_some() && !self.opts.debug.column_info {
            return orig;
        }
        let l = match self.di_location(loc, scope) {
            Some(l) => l,
            None => self.di_location_at(0, 0, scope),
        };
        self.f.fir.set_dbg_loc(Some(l));
        orig
    }

    /// clang `ApplyDebugLocation::CreateDefaultArtificial(CGF, Loc)`.
    pub(crate) fn dbg_apply_default_artificial(&mut self, loc: Loc) -> Option<Option<MetadataId>> {
        self.dbg_apply(loc)
    }

    /// clang `~ApplyDebugLocation`.
    #[inline]
    pub(crate) fn dbg_restore(&mut self, saved: Option<Option<MetadataId>>) {
        if let Some(l) = saved {
            self.f.fir.set_dbg_loc(l);
        }
    }

    /// clang `LexicalScope`'s constructor (`EmitLexicalBlockStart`): the
    /// location becomes `begin` in the enclosing scope, then a block
    /// starting there is pushed (not with line tables only).
    pub(crate) fn dbg_block_start(&mut self, begin: Loc) {
        let Some(d) = self.f.fir.dbg.as_deref() else { return };
        let scope = d.scope();
        let (line, col) = self.di_line_col(begin);
        let l = self.di_location_at(line, col, scope);
        self.f.fir.set_dbg_loc(Some(l));
        if !self.opts.debug.reduced() && self.opts.debug.kind != DebugInfoKind::LineTablesOnly {
            // Limited and above push blocks; see below.
        }
        if self.opts.debug.kind <= DebugInfoKind::LineTablesOnly {
            return;
        }
        let file = self.di_file(begin);
        let mut f = Fields::default().n("scope", Some(scope)).n("file", Some(file)).u("line", u64::from(line));
        if col != 0 {
            f = f.u("column", u64::from(col));
        }
        let b = self.m.md_specialized("DILexicalBlock", true, f.0);
        if let Some(d) = self.f.fir.dbg.as_deref_mut() {
            d.scopes.push(b);
        }
    }

    /// clang `~LexicalScope`'s `EmitLexicalBlockEnd`: the location becomes
    /// `end` in the block, which is popped.
    pub(crate) fn dbg_block_end(&mut self, end: Loc) {
        if !self.dbg_on() {
            return;
        }
        self.dbg_emit_location(end);
        if self.opts.debug.kind <= DebugInfoKind::LineTablesOnly {
            return;
        }
        if let Some(d) = self.f.fir.dbg.as_deref_mut()
            && d.scopes.len() > 1
        {
            d.scopes.pop();
        }
    }

    /// (line, column) of `loc` as locations print it (0, 0 if invalid).
    pub(crate) fn di_line_col(&self, loc: Loc) -> (u32, u32) {
        match self.di.as_deref().and_then(|d| d.sm.presumed_line_col(loc)) {
            Some(p) => (p.line, if self.opts.debug.column_info { p.col } else { 0 }),
            None => (0, 0),
        }
    }
}

#[cold]
fn unreachable_md() -> ! {
    unreachable!("debug information queried at -g0")
}

/// clang `createFile`'s directory/file split of `file` against the
/// compilation directory `cur`: an absolute name loses the common prefix
/// unless it is only the root; a relative one keeps `cur` as directory.
fn split_against(file: &str, cur: &str, was_absolute: bool) -> (String, String) {
    if !file.starts_with('/') {
        let dir = if was_absolute { String::new() } else { cur.to_owned() };
        return (dir, file.to_owned());
    }
    let fc: Vec<&str> = components(file);
    let cc: Vec<&str> = components(cur);
    let common = fc.iter().zip(&cc).take_while(|(a, b)| a == b).count();
    if common <= 1 {
        return (String::new(), file.to_owned());
    }
    (join(&fc[..common]), join(&fc[common..]).trim_start_matches('/').to_owned())
}

/// `llvm::sys::path` components: the root `/`, then each non-empty name
/// (`.` kept, as LLVM keeps it).
fn components(p: &str) -> Vec<&str> {
    let mut v = Vec::new();
    if p.starts_with('/') {
        v.push("/");
    }
    v.extend(p.split('/').filter(|c| !c.is_empty()));
    v
}

/// `llvm::sys::path::append` of `parts`.
fn join(parts: &[&str]) -> String {
    let mut s = String::new();
    for p in parts {
        if *p == "/" {
            s.push('/');
            continue;
        }
        if !s.is_empty() && !s.ends_with('/') {
            s.push('/');
        }
        s.push_str(p);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::split_against;

    #[test]
    fn file_split_matches_clang() {
        let s = |f, c| split_against(f, c, f.starts_with('/'));
        assert_eq!(s("a.cpp", "/w"), ("/w".into(), "a.cpp".into()));
        assert_eq!(s("../t/x.cpp", "/w/c"), ("/w/c".into(), "../t/x.cpp".into()));
        assert_eq!(s("/w/c/x.h", "/w/c"), ("/w/c".into(), "x.h".into()));
        assert_eq!(s("/w/d/x.h", "/w/c"), ("/w".into(), "d/x.h".into()));
        assert_eq!(s("/usr/include/x.h", "/w/c"), (String::new(), "/usr/include/x.h".into()));
        // A remapped relative name of an absolute path has no directory.
        assert_eq!(split_against("x/a.cpp", "/w", true), (String::new(), "x/a.cpp".into()));
    }
}
