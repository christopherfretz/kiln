//! clang 19 `CoverageMappingModuleGen` (WP-1003, spec/codegen.md
//! "Instrumentation"): the module's coverage file table, the
//! `__covrec_` function records in `__llvm_covfun`, the translation
//! unit's `__llvm_coverage_mapping` header and filenames in
//! `__llvm_covmap`, and the zero records of the functions defined but
//! never emitted (`DeferredEmptyCoverageMappingDecls`, named in
//! `__llvm_coverage_names`).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted};
use kiln_llvm::{GlobalId, Linkage, TypeId as LTy, Value, Visibility};
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Interner;
use kiln_source::{FileId, Loc, SourceManager, Span};

use super::encode::{md5_hash, write_filenames};
use super::mapping::{self, Files, Locs};
use super::skipped::{SkippedRange, scan_entry};
use crate::module::Cg;

/// One `__covrec_` record (clang `FunctionInfo`).
#[derive(Debug)]
struct Rec {
    name_hash: u64,
    func_hash: u64,
    mapping: Vec<u8>,
    used: bool,
}

/// The coverage state of the module.
#[derive(Default)]
pub(crate) struct CovModule {
    /// `FileEntries`: the global index (from 1) of each file.
    files: FxHashMap<FileId, u32>,
    names: Vec<FileId>,
    /// The skipped ranges of each file entry, by its base location.
    skipped: FxHashMap<Loc, Vec<SkippedRange>>,
    records: Vec<Rec>,
    /// `FunctionNames`: the `__profn_` of each unused function's record.
    unused: Vec<GlobalId>,
    /// `DeferredEmptyCoverageMappingDecls`: insertion order and whether
    /// the function still needs its zero record.
    deferred: FxHashMap<DeclId, bool>,
    deferred_order: Vec<DeclId>,
    /// How many of `DeclArena::fn_def_insts` were deferred.
    insts_seen: usize,
    /// The lexer's identifier table for the location queries (taken
    /// while a mapping is built).
    interner: Option<Box<Interner>>,
}

impl std::fmt::Debug for CovModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CovModule").field("records", &self.records.len()).finish_non_exhaustive()
    }
}

/// [`Files`] over the module's table.
struct Table<'a> {
    files: &'a mut FxHashMap<FileId, u32>,
    names: &'a mut Vec<FileId>,
    skipped: &'a mut FxHashMap<Loc, Vec<SkippedRange>>,
    sm: &'a SourceManager,
    pp: &'a [Span],
    macros: &'a [(Span, bool)],
}

impl Files for Table<'_> {
    fn global_id(&mut self, file: FileId) -> u32 {
        let n = self.files.len() as u32 + 1;
        *self.files.entry(file).or_insert_with(|| {
            self.names.push(file);
            n
        })
    }

    fn skipped(&mut self, file: FileId, base: Loc) -> &[SkippedRange] {
        let (sm, pp, macros) = (self.sm, self.pp, self.macros);
        self.skipped.entry(base).or_insert_with(|| scan_entry(sm, file, base, pp, macros))
    }
}

/// LLVM `sys::path::remove_dots(.., true)` on a POSIX path: `.`
/// components and `x/..` pairs go, separators collapse.
fn remove_dots(p: &str) -> String {
    let abs = p.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." if parts.last().is_some_and(|l| *l != "..") => {
                parts.pop();
            }
            ".." if abs => {}
            c => parts.push(c),
        }
    }
    let s = parts.join("/");
    if abs { format!("/{s}") } else { s }
}

impl Cg<'_> {
    /// clang `CoverageMappingModuleGen::normalizeFilename`: dots removed,
    /// then the last matching `-fcoverage-prefix-map` applied.
    fn cov_normalize(&self, name: &str) -> String {
        let mut p = remove_dots(name);
        for (from, to) in self.opts.coverage.prefix_map.iter().rev() {
            if let Some(rest) = p.strip_prefix(from.as_str()) {
                p = format!("{to}{rest}");
                break;
            }
        }
        p
    }

    /// clang `AddDeferredUnusedCoverageMapping`: function definition `d`
    /// gets a zero record unless it is emitted.
    pub(crate) fn cov_defer_unused(&mut self, d: DeclId) {
        if !self.opts.coverage.mapping {
            return;
        }
        let ast = self.ast;
        let n = ast.decls.get(d);
        if !matches!(
            n.kind(),
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConversion
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
        ) || ast.decls.function(d).is_none_or(|f| f.body.is_none())
            || self.sm.is_some_and(|sm| sm.is_in_system_header(n.loc()))
        {
            return;
        }
        if let std::collections::hash_map::Entry::Vacant(v) = self.cov.deferred.entry(d) {
            v.insert(true);
            self.cov.deferred_order.push(d);
        }
    }

    /// The function definitions Sema instantiated before top-level
    /// declaration `upto` (all of them for `u32::MAX`): clang hands each
    /// to `EmitTopLevelDecl` when it is instantiated, which defers its
    /// zero record (`AddDeferredUnusedCoverageMapping`), so one used only
    /// in constant evaluation still gets one (WP-1083).
    pub(crate) fn cov_defer_insts(&mut self, upto: u32) {
        if !self.opts.coverage.mapping {
            return;
        }
        let ast = self.ast;
        let insts = ast.decls.fn_def_insts();
        while let Some(&(f, k)) = insts.get(self.cov.insts_seen) {
            if k > upto {
                break;
            }
            self.cov.insts_seen += 1;
            let n = ast.decls.get(f);
            // `EmitTopLevelDecl` defers functions and methods only (a
            // constructor or destructor gets one from
            // `HandleInlineFunctionDefinition`, never instantiated). A
            // std builtin (`std::move`, ...) is never referenced by its
            // calls, so clang never instantiates it for them.
            if !matches!(
                n.kind(),
                DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion
            ) || n.has(DeclFlags::INVALID)
                || n.has(DeclFlags::DEPENDENT_CTX)
                || ast.decls.function(f).is_some_and(|x| x.spec.is_immediate())
                || self.is_std_builtin(f)
            {
                continue;
            }
            self.cov_defer_unused(f);
        }
    }

    /// clang `HandleInlineFunctionDefinition`'s coverage part: the
    /// member functions defined in class `rd` (and its nested classes)
    /// get zero records unless emitted. A closure type is not a
    /// top-level declaration in clang, so an unused lambda gets none
    /// (WP-1083). Only a definition the parser saw counts (clang calls it
    /// from `ActOnFinishInlineFunctionDef`): a member of a class template
    /// specialization (`extern template struct X<int>;` lists one in the
    /// translation unit) was instantiated, and its record is deferred
    /// where its instantiation ends, in `cov_defer_insts` (WP-1107).
    pub(crate) fn cov_defer_class(&mut self, rd: DeclId, depth: usize) {
        let ast = self.ast;
        if !self.opts.coverage.mapping || depth > 64 || ast.closure_lambda(rd).is_some() {
            return;
        }
        let members: Vec<DeclId> = ast.decls.members(rd).collect();
        for m in members {
            let n = ast.decls.get(m);
            if n.has(DeclFlags::INVALID)
                || n.has(DeclFlags::DEPENDENT_CTX)
                || n.has(DeclFlags::IMPLICIT)
                || ast.instantiated_from(m).is_some()
            {
                continue;
            }
            match n.kind() {
                DeclKind::CXXRecord => self.cov_defer_class(m, depth + 1),
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConversion
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor => {
                    let Some(f) = ast.decls.function(m) else { continue };
                    if f.body.is_some()
                        && f.spec.defaulted() == Defaulted::No
                        && !self.is_templated(m)
                    {
                        self.cov_defer_unused(m);
                    }
                }
                _ => {}
            }
        }
    }

    /// clang `ClearUnusedCoverageMapping`: `d` (and the pattern it was
    /// instantiated from) is emitted.
    pub(crate) fn cov_clear_unused(&mut self, d: DeclId) {
        if !self.opts.coverage.mapping {
            return;
        }
        let mut c = Some(d);
        for _ in 0..8 {
            let Some(x) = c else { break };
            if self.cov.deferred.insert(x, false).is_none() {
                self.cov.deferred_order.push(x);
            }
            c = self.ast.instantiated_from(x).filter(|&p| p != x);
        }
    }

    /// clang `emitCounterRegionMapping` / `emitEmptyCounterMapping`: the
    /// mapping of `d` (with `rc`), or its zero mapping without.
    pub(crate) fn cov_mapping(
        &mut self,
        d: DeclId,
        rc: Option<&super::pgo::RegionCounters>,
    ) -> Option<Vec<u8>> {
        let sm = self.sm?;
        let ast = self.ast;
        let body = ast.decls.function(d)?.body?;
        if sm.is_in_system_header(ast.stmts.get(body).loc()) {
            return None;
        }
        let interner = self.cov.interner.take().unwrap_or_else(|| Box::new(Interner::new()));
        let l = Locs { sm, interner: &interner };
        let opts = self.opts;
        let out = if let Some(rc) = rc {
            let mut fold = |e| self.fold_int(e);
            let mut b = mapping::Builder::new(ast, l, rc, &mut fold, opts.clang_compat);
            b.function(d);
            let mut parts = b.into_parts();
            let mut t = Table {
                files: &mut self.cov.files,
                names: &mut self.cov.names,
                skipped: &mut self.cov.skipped,
                sm,
                pp: &opts.coverage.skipped,
                macros: &opts.coverage.expanded,
            };
            mapping::finish(l, &mut parts.regions, &parts.exprs, true, &mut t)
        } else {
            let mut t = Table {
                files: &mut self.cov.files,
                names: &mut self.cov.names,
                skipped: &mut self.cov.skipped,
                sm,
                pp: &opts.coverage.skipped,
                macros: &opts.coverage.expanded,
            };
            mapping::empty(ast, l, d, &mut t)
        };
        self.cov.interner = Some(interner);
        out
    }

    /// clang `addFunctionMappingRecord`.
    pub(crate) fn cov_add_record(
        &mut self,
        name: &str,
        name_var: GlobalId,
        func_hash: u64,
        mapping: Vec<u8>,
        used: bool,
    ) {
        self.cov.records.push(Rec {
            name_hash: md5_hash(name.as_bytes()),
            func_hash,
            mapping,
            used,
        });
        if !used {
            self.cov.unused.push(name_var);
        }
    }

    /// clang `EmitDeferredUnusedCoverageMappings` and
    /// `CoverageMappingModuleGen::emit`, before `emitLLVMUsed`.
    pub(crate) fn cov_emit(&mut self) {
        if !self.opts.coverage.mapping || !self.opts.coverage.instr {
            return;
        }
        self.cov_defer_insts(u32::MAX);
        let order = std::mem::take(&mut self.cov.deferred_order);
        for d in order {
            if self.cov.deferred.get(&d) != Some(&true) {
                continue;
            }
            let Some(mapping) = self.cov_mapping(d, None) else { continue };
            let ast = self.ast;
            let first = ast.first(d);
            let name = match ast.decls.get(d).kind() {
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    kiln_abi::mangle::mangle_structor(
                        ast,
                        &mut self.anon,
                        first,
                        kiln_abi::mangle::Structor::Base,
                    )
                }
                _ => kiln_abi::mangle::mangle_decl(ast, &mut self.anon, first),
            };
            let linkage = self.fn_def_linkage(d);
            let (g, pgo) = self.prof_name_var(&name, linkage);
            self.cov_add_record(&pgo, g, 0, mapping, false);
        }
        if self.cov.records.is_empty() {
            return;
        }
        // The filenames: the compilation directory, then each file.
        let sm = self.sm;
        let mut strs = vec![self.cov_normalize(&self.opts.coverage.comp_dir.clone())];
        let names = std::mem::take(&mut self.cov.names);
        for f in names {
            let n = sm.map_or(String::new(), |sm| sm.file_name(f).to_owned());
            strs.push(self.cov_normalize(&n));
        }
        let filenames = write_filenames(&strs);
        let tu_hash = md5_hash(&filenames);
        let records = std::mem::take(&mut self.cov.records);
        for r in records {
            let mut name = format!("__covrec_{:X}", r.name_hash);
            if r.used {
                name.push('u');
            }
            let bytes = self.m.const_bytes(&r.mapping);
            let bty = self.m.const_type(bytes);
            let sty =
                self.m.types.struct_literal(&[LTy::I64, LTy::I32, LTy::I64, LTy::I64, bty], true);
            let vals = [
                self.m.const_int(LTy::I64, i128::from(r.name_hash as i64)),
                self.m.const_int(LTy::I32, r.mapping.len() as i128),
                self.m.const_int(LTy::I64, i128::from(r.func_hash as i64)),
                self.m.const_int(LTy::I64, i128::from(tu_hash as i64)),
                bytes,
            ];
            let Ok(init) = self.m.const_struct(sty, &vals) else { continue };
            let g = self.m.add_global(&name, sty);
            let _ = self.m.set_initializer(g, Some(init));
            let c = self.m.comdat(&name, kiln_llvm::ComdatKind::Any);
            let gv = self.m.global_mut(g);
            gv.linkage = Linkage::LinkOnceOdr;
            gv.visibility = Visibility::Hidden;
            gv.is_constant = true;
            gv.section = Some("__llvm_covfun".into());
            gv.align = Some(8);
            gv.comdat = Some(c);
            self.gi_add_used(Value::global(g));
        }
        // `__llvm_coverage_mapping`: { { 0, filenames size, 0, version } }.
        let hty = self.m.types.struct_literal(&[LTy::I32; 4], false);
        let hv = [
            self.m.const_int(LTy::I32, 0),
            self.m.const_int(LTy::I32, filenames.len() as i128),
            self.m.const_int(LTy::I32, 0),
            self.m.const_int(LTy::I32, 6),
        ];
        let bytes = self.m.const_bytes(&filenames);
        let bty = self.m.const_type(bytes);
        let sty = self.m.types.struct_literal(&[hty, bty], false);
        if let Ok(h) = self.m.const_struct(hty, &hv)
            && let Ok(init) = self.m.const_struct(sty, &[h, bytes])
        {
            let g = self.m.add_global("__llvm_coverage_mapping", sty);
            let _ = self.m.set_initializer(g, Some(init));
            let gv = self.m.global_mut(g);
            gv.linkage = Linkage::Private;
            gv.is_constant = true;
            gv.section = Some("__llvm_covmap".into());
            gv.align = Some(8);
            self.gi_add_used(Value::global(g));
        }
        // `__llvm_coverage_names`: the unused functions' names.
        let unused = std::mem::take(&mut self.cov.unused);
        if !unused.is_empty() {
            let elems: Vec<Value> = unused.iter().map(|&g| Value::global(g)).collect();
            if let Ok(init) = self.m.const_array(LTy::PTR, &elems) {
                let ty = self.m.const_type(init);
                let g = self.m.add_global("__llvm_coverage_names", ty);
                let _ = self.m.set_initializer(g, Some(init));
                let gv = self.m.global_mut(g);
                gv.linkage = Linkage::Internal;
                gv.is_constant = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::remove_dots;

    #[test]
    fn dots() {
        assert_eq!(remove_dots("/a/./b/../c//d"), "/a/c/d");
        assert_eq!(remove_dots("a/../../b"), "../b");
        assert_eq!(remove_dots("/../a"), "/a");
    }
}
