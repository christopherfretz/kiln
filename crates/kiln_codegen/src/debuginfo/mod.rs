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
    pub fn language_for(
        cplusplus: bool,
        at_least_cxx11: bool,
        at_least_cxx14: bool,
    ) -> &'static str {
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

pub(crate) mod func;
mod md5;
mod stmt;
mod types;

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
    /// A node field, left out when null.
    pub fn n(mut self, k: &str, v: Option<MetadataId>) -> Self {
        if v.is_some() {
            self.0.push((k.into(), MdField::Node(v)));
        }
        self
    }
    /// A node field printed even when null (`baseType: null`).
    pub fn nn(mut self, k: &str, v: Option<MetadataId>) -> Self {
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
    /// `spFlags:`, always printed (none reads as `isDefinition: true`).
    pub fn sp_flags(self, f: &[&str]) -> Self {
        if f.is_empty() { self.raw("spFlags", "0") } else { self.raw("spFlags", &f.join(" | ")) }
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
    /// Classes homed by a constructor (clang `completeUnusedClass`),
    /// kept in the unit's retained types.
    pub retained_classes: Vec<DeclId>,
    /// Function scopes by first declaration (clang `RegionMap`).
    pub regions: FxHashMap<DeclId, MetadataId>,
    /// `__vtbl_ptr_type *`.
    pub vtbl_ptr: Option<MetadataId>,
    /// The first construct debug information cannot describe yet.
    pub error: Option<crate::CodegenError>,
    /// Member function declarations by first declaration.
    pub decl_sps: FxHashMap<DeclId, MetadataId>,
    /// Namespaces by first declaration.
    pub namespaces: FxHashMap<DeclId, MetadataId>,
    /// Global variable expressions by first declaration.
    pub gves: FxHashMap<DeclId, MetadataId>,
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
        let cu_file = self.di_create_file(name, checksum.as_deref(), true);
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
        self.di = Some(Box::new(Di {
            sm,
            dbg,
            cu,
            cu_file,
            empty_expr,
            files: FxHashMap::default(),
            locs: FxHashMap::default(),
            types: FxHashMap::default(),
            classes: FxHashMap::default(),
            class_order: Vec::new(),
            completed: FxHashSet::default(),
            retained_classes: Vec::new(),
            regions: FxHashMap::default(),
            vtbl_ptr: None,
            error: None,
            decl_sps: FxHashMap::default(),
            namespaces: FxHashMap::default(),
            gves: FxHashMap::default(),
            static_members: FxHashMap::default(),
            enums: Vec::new(),
            retained: Vec::new(),
            globals: Vec::new(),
            imported: Vec::new(),
        }));
    }

    /// clang `CGDebugInfo::createFile`: the remapped name split against
    /// the remapped compilation directory.
    ///
    /// `cu`: the compile unit's file, which `CreateCompileUnit` names as
    /// given, in the compilation directory (no common-prefix split).
    fn di_create_file(&mut self, name: &str, checksum: Option<&str>, cu: bool) -> MetadataId {
        let o = &self.opts.debug;
        let file = o.remap(name);
        let cur = o.remap(&o.comp_dir);
        let (dir, file) =
            if cu { (cur, file) } else { split_against(&file, &cur, name.starts_with('/')) };
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
        let checksum =
            (p.file_name == sm.file_name(p.file)).then(|| md5::hex(sm.text(p.file).as_bytes()));
        let f = self.di_create_file(&name, checksum.as_deref(), false);
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

    /// clang `CGDebugInfo::finalize`: never-completed class placeholders
    /// become declarations; the compile unit gets its enums, retained
    /// types, globals and imported entities.
    pub(crate) fn finish_debug_info(&mut self) {
        let Some(order) = self.di.as_deref().map(|d| d.class_order.clone()) else { return };
        for rd in order {
            let (done, p) = match self.di.as_deref() {
                Some(d) => (d.completed.contains(&rd), d.classes[&rd]),
                None => return,
            };
            if !done {
                let node = self.di_class_decl(rd);
                let _ = self.m.md_replace(p, node);
            }
        }
        let Some(d) = self.di.as_deref_mut() else { return };
        let homed: Vec<DeclId> = std::mem::take(&mut d.retained_classes);
        let mut retained = std::mem::take(&mut d.retained);
        for rd in homed {
            if let Some(&p) = self.di.as_deref().and_then(|d| d.classes.get(&rd))
                && !retained.contains(&p)
            {
                retained.push(p);
            }
        }
        let Some(d) = self.di.as_deref() else { return };
        let (cu, lists) = (
            d.cu,
            [
                ("enums", d.enums.clone()),
                ("retainedTypes", retained),
                ("globals", d.globals.clone()),
                ("imports", d.imported.clone()),
            ],
        );
        for (k, v) in lists {
            if !v.is_empty() {
                let ops: Vec<Option<MetadataId>> = v.into_iter().map(Some).collect();
                let t = self.m.md_tuple(&ops);
                let _ = self.m.md_set_field(cu, k, MdField::Node(Some(t)));
            }
        }
    }

    /// The first unsupported construct met by debug information.
    pub(crate) fn take_debug_info_error(&mut self) -> Option<crate::CodegenError> {
        self.di.as_deref_mut().and_then(|d| d.error.take())
    }

    // ---- function locations (clang `CodeGenFunction` + `CGDebugInfo`) ---

    /// Whether the current function has debug locations.
    #[inline]
    pub(crate) fn dbg_on(&self) -> bool {
        self.f.fir.dbg.is_some()
    }

    /// clang `CGDebugInfo::setLocation` + `EmitLocation`: a valid `loc`
    /// becomes `CurLoc`; the builder location becomes `CurLoc` in the
    /// innermost scope (nothing while `CurLoc` is invalid).
    pub(crate) fn dbg_emit_location(&mut self, loc: Loc) {
        let Some(d) = self.f.fir.dbg.as_deref_mut() else { return };
        if loc != Loc::INVALID {
            d.cur_loc = loc;
        }
        let (cur, scope) = (d.cur_loc, d.scope());
        if let Some(l) = self.di_location(cur, scope) {
            self.f.fir.set_dbg_loc(Some(l));
        }
    }

    /// clang `EmitStopPoint`.
    pub(crate) fn dbg_stop_point(&mut self, loc: Loc) {
        if !self.dbg_on() {
            return;
        }
        self.dbg_emit_location(loc);
        if let Some(d) = self.f.fir.dbg.as_deref_mut() {
            d.last_stop = loc;
        }
    }

    /// clang `ApplyDebugLocation(CGF, Loc)`'s constructor: returns the
    /// location to restore (`None` when the function has none).
    /// Expression locations are always on (clang
    /// `getExpressionLocationsEnabled` without CodeView).
    #[inline]
    pub(crate) fn dbg_apply(&mut self, loc: Loc) -> Option<Option<MetadataId>> {
        let d = self.f.fir.dbg.as_deref()?;
        let orig = d.loc;
        if loc == Loc::INVALID {
            let scope = d.scope();
            let l = self.di_location_at(0, 0, scope);
            self.f.fir.set_dbg_loc(Some(l));
        } else {
            self.dbg_emit_location(loc);
        }
        Some(orig)
    }

    /// clang `ApplyDebugLocation::CreateDefaultArtificial(CGF, Loc)`:
    /// only when the builder has a location.
    pub(crate) fn dbg_apply_default_artificial(&mut self, loc: Loc) -> Option<Option<MetadataId>> {
        let d = self.f.fir.dbg.as_deref()?;
        if d.loc.is_none() {
            return Some(None);
        }
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
    /// starting there is pushed (not with line tables only). Returns
    /// whether a scope was entered (to pass to [`Self::dbg_block_end`]).
    pub(crate) fn dbg_block_start(&mut self, begin: Loc) -> bool {
        let Some(d) = self.f.fir.dbg.as_deref_mut() else { return false };
        if begin != Loc::INVALID {
            d.cur_loc = begin;
        }
        let (scope, cur) = (d.scope(), d.cur_loc);
        let (line, col) = self.di_line_col(begin);
        let l = self.di_location_at(line, col, scope);
        self.f.fir.set_dbg_loc(Some(l));
        if self.opts.debug.kind <= DebugInfoKind::LineTablesOnly {
            return true;
        }
        let file = self.di_file(cur);
        let (line, col) = self.di_line_col(cur);
        let mut f = Fields::default()
            .n("scope", Some(scope))
            .n("file", Some(file))
            .u("line", u64::from(line));
        if col != 0 {
            f = f.u("column", u64::from(col));
        }
        let b = self.m.md_specialized("DILexicalBlock", true, f.0);
        if let Some(d) = self.f.fir.dbg.as_deref_mut() {
            d.scopes.push(b);
        }
        true
    }

    /// clang `~LexicalScope`'s `EmitLexicalBlockEnd`: the location becomes
    /// `end` in the block, which is popped.
    pub(crate) fn dbg_block_end(&mut self, started: bool, end: Loc) {
        if !started || !self.dbg_on() {
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
