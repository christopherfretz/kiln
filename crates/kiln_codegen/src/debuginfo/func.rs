//! Function subprograms and variables (clang `emitFunctionStart`,
//! `EmitDeclare`, `EmitGlobalVariable`).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::{DeclId, QualType, StmtId};
use kiln_llvm::{FuncId, GlobalId, MdField, MetadataId};
use kiln_source::Loc;

use super::{DebugInfoKind, Fields};
use crate::fir::{DbgRec, FirDbg, V};
use crate::module::Cg;

/// How a variable's storage is described (clang `EmitDeclare`'s
/// `ArgNo` and `UsePointerValue`).
#[derive(Clone, Copy)]
pub(crate) struct VarSlot {
    pub addr: V,
    /// The 1-based argument number of a parameter.
    pub arg: Option<u32>,
    /// The slot holds a pointer to the variable (`DW_OP_deref`).
    pub deref: bool,
}

impl Cg<'_> {
    pub(crate) fn di_reduced(&self) -> bool {
        self.di.is_some() && self.opts.debug.kind > DebugInfoKind::LineTablesOnly
    }

    /// clang `StartFunction`'s `emitFunctionStart` for the definition `d`
    /// emitted into `f` (a thunk has no declaration of its own): the
    /// distinct `DISubprogram`, attached to `f`, and the function's
    /// location state. A `nodebug` function gets none.
    pub(crate) fn di_fn_start(&mut self, d: DeclId, f: FuncId, thunk: bool, body: Option<StmtId>) {
        let Some(di) = self.di.as_deref() else { return };
        let (dbg, cu) = (di.dbg, di.cu);
        let ast = self.ast;
        if self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
        let reduced = self.di_reduced();
        let first = ast.first(d);
        let n = ast.decls.get(d);
        let loc = n.loc();
        let unit = self.di_file(loc);
        let fname = self.m.func(f).name().to_owned();
        let kind = n.kind();
        let method = matches!(
            kind,
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        );
        let (mut fl, mut sp): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
        let fake = |cg: &mut Self| {
            let t = cg.m.md_tuple(&[]);
            cg.m.md_specialized("DISubroutineType", false, Fields::default().n("types", Some(t)).0)
        };
        let mut f_ = Fields::default();
        let (scope, ty, decl, tparams);
        if thunk {
            f_ = f_.s("linkageName", &fname);
            fl.extend(["DIFlagArtificial", "DIFlagThunk"]);
            (scope, ty, decl, tparams) = (unit, fake(self), None, None);
        } else {
            let name = self.di_fn_name(d);
            f_ = f_.s("name", &name);
            // clang keeps the linkage name for gcov's notes too (WP-941).
            if (reduced || self.opts.coverage.gcov.is_some()) && fname != name {
                f_ = f_.s("linkageName", &fname);
            }
            fl.push("DIFlagPrototyped");
            let ctx = n.ctx();
            scope = match ctx.map(|c| (c, ast.decls.get(c).kind())) {
                Some((c, DeclKind::Namespace)) if reduced => {
                    self.di_scope_nocu(d).unwrap_or_else(|| self.di_file(ast.decls.get(c).loc()))
                }
                Some((c, DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization))
                    if reduced =>
                {
                    ast.decls.get(c).ty().and_then(|q| self.di_type(q)).unwrap_or(unit)
                }
                _ => unit,
            };
            if reduced {
                if self.di_fn_noreturn(d) {
                    fl.push("DIFlagNoReturn");
                }
                tparams = self.di_template_params(d);
            } else {
                tparams = None;
            }
            if n.has(DeclFlags::IMPLICIT) || self.has_fn_attr(d, AttrKind::ARTIFICIAL) {
                fl.push("DIFlagArtificial");
            }
            ty = if !reduced {
                fake(self)
            } else if method {
                match self.di_method_type(d) {
                    Some(t) => t,
                    None => fake(self),
                }
            } else {
                match self.di_free_fn_type(d) {
                    Some(t) => t,
                    None => fake(self),
                }
            };
            decl = if reduced && method { self.di_method_decl(d) } else { None };
        }
        // clang `getCallSiteRelatedAttrs`: none for location tracking.
        if self.opts.opt_level > 0
            && self.opts.debug.kind != DebugInfoKind::LocTrackingOnly
            && self.opts.debug.dwarf_version.unwrap_or(5) >= 5
        {
            fl.push("DIFlagAllCallsDescribed");
        }
        if self.m.func(f).linkage.is_local() {
            sp.push("DISPFlagLocalToUnit");
        }
        if self.opts.opt_level > 0 {
            sp.push("DISPFlagOptimized");
        }
        sp.push("DISPFlagDefinition");
        let line = self.di_line(loc);
        let scope_loc = if thunk { loc } else { body.map_or(loc, |b| ast.stmts.get(b).loc()) };
        let scope_line = self.di_line(scope_loc);
        f_ = f_
            .n("scope", Some(scope))
            .n("file", Some(unit))
            .u("line", u64::from(line))
            .n("type", Some(ty));
        f_ = f_.u("scopeLine", u64::from(scope_line)).flags("flags", &fl).sp_flags(&sp);
        f_ = f_.n("unit", Some(cu)).n("templateParams", tparams).n("declaration", decl);
        let s = self.m.md_specialized("DISubprogram", true, f_.0);
        self.m.func_mut(f).add_metadata(dbg, s);
        if let Some(di) = self.di.as_deref_mut() {
            di.regions.insert(first, s);
        }
        let mut fd = FirDbg::new(dbg, s);
        fd.eh_loc = self.di_body_end(body);
        self.f.fir.dbg = Some(Box::new(fd));
    }

    /// clang `StartFunction(GlobalDecl(), ...)` of a coroutine's private
    /// `await_suspend` wrapper `f` (CGCoroutine.cpp): an artificial
    /// `DISubprogram` with only a linkage name, in the main file. gcov
    /// instruments only functions that have one.
    pub(crate) fn di_wrapper_start(&mut self, f: FuncId) {
        let Some(di) = self.di.as_deref() else { return };
        let (dbg, cu) = (di.dbg, di.cu);
        let file = self.di_file(Loc::INVALID);
        let fname = self.m.func(f).name().to_owned();
        let types = self.m.md_tuple(&[]);
        let ty = self.m.md_specialized(
            "DISubroutineType",
            false,
            Fields::default().n("types", Some(types)).0,
        );
        let mut fl = vec!["DIFlagArtificial"];
        if self.opts.opt_level > 0
            && self.opts.debug.kind != DebugInfoKind::LocTrackingOnly
            && self.opts.debug.dwarf_version.unwrap_or(5) >= 5
        {
            fl.push("DIFlagAllCallsDescribed");
        }
        let mut sp = vec!["DISPFlagLocalToUnit"];
        if self.opts.opt_level > 0 {
            sp.push("DISPFlagOptimized");
        }
        sp.push("DISPFlagDefinition");
        let f_ = Fields::default()
            .s("linkageName", &fname)
            .n("scope", Some(file))
            .n("file", Some(file))
            .n("type", Some(ty))
            .flags("flags", &fl)
            .sp_flags(&sp)
            .n("unit", Some(cu));
        let s = self.m.md_specialized("DISubprogram", true, f_.0);
        self.m.func_mut(f).add_metadata(dbg, s);
        self.f.fir.dbg = Some(Box::new(FirDbg::new(dbg, s)));
    }

    /// clang `StartFunction` of a compiler-generated initialization or
    /// cleanup function `f` ([basic.start.dynamic], [basic.start.term];
    /// WP-1445): with `var`, a `GlobalDecl(var, DynamicInitKind)`, so the
    /// artificial subprogram is named after `f` (`__cxx_global_var_init`,
    /// `__cxx_global_array_dtor`) and has the type `void(params)`;
    /// without, `GlobalDecl()` (`_GLOBAL__sub_I_*`, `__tls_init`): only a
    /// linkage name and no types. `ptr_param`: one `void *` parameter.
    /// No line: the location is invalid, so
    /// the file is the main file. `CurEHLocation` is `var`'s begin. A
    /// `nodebug` variable's function gets none. The caller then applies
    /// the artificial location ([`Self::dbg_apply`] of an invalid one).
    pub(crate) fn di_init_fn_start(&mut self, f: FuncId, var: Option<DeclId>, ptr_param: bool) {
        let Some(di) = self.di.as_deref() else { return };
        let (dbg, cu) = (di.dbg, di.cu);
        if var.is_some_and(|v| self.has_fn_attr(v, AttrKind::NODEBUG)) {
            return;
        }
        let file = self.di_file(Loc::INVALID);
        let fname = self.m.func(f).name().to_owned();
        let mut types = Vec::new();
        if var.is_some() && self.di_reduced() {
            types.push(None);
            if ptr_param {
                types.push(Some(self.di_void_ptr()));
            }
        }
        let types = self.m.md_tuple(&types);
        let ty = self.m.md_specialized(
            "DISubroutineType",
            false,
            Fields::default().n("types", Some(types)).0,
        );
        let mut fl = vec!["DIFlagArtificial"];
        if self.opts.opt_level > 0
            && self.opts.debug.kind != DebugInfoKind::LocTrackingOnly
            && self.opts.debug.dwarf_version.unwrap_or(5) >= 5
        {
            fl.push("DIFlagAllCallsDescribed");
        }
        let mut sp = Vec::new();
        if self.m.func(f).linkage.is_local() {
            sp.push("DISPFlagLocalToUnit");
        }
        if self.opts.opt_level > 0 {
            sp.push("DISPFlagOptimized");
        }
        sp.push("DISPFlagDefinition");
        let f_ = if var.is_some() {
            Fields::default().s("name", &fname)
        } else {
            Fields::default().s("linkageName", &fname)
        };
        let f_ = f_
            .n("scope", Some(file))
            .n("file", Some(file))
            .n("type", Some(ty))
            .flags("flags", &fl)
            .sp_flags(&sp)
            .n("unit", Some(cu));
        let s = self.m.md_specialized("DISubprogram", true, f_.0);
        self.m.func_mut(f).add_metadata(dbg, s);
        let mut fd = FirDbg::new(dbg, s);
        if let Some(v) = var {
            let ast = self.ast;
            fd.eh_loc = ast.decls.cold.begin_loc.get(v).copied().unwrap_or(ast.decls.get(v).loc());
        }
        self.f.fir.dbg = Some(Box::new(fd));
    }

    /// The unnamed artificial `void *` parameter of a
    /// `__cxx_global_array_dtor` (clang `generateDestroyHelper`'s
    /// `ImplicitParamDecl`), argument 1 in `addr`.
    pub(crate) fn di_declare_void_ptr_param(&mut self, addr: V) {
        if !self.di_declaring() {
            return;
        }
        let ty = Some(self.di_void_ptr());
        let _ = self.di_declare(None, "", ty, VarSlot { addr, arg: Some(1), deref: false }, false);
    }

    /// `void *` (`DW_TAG_pointer_type` with a null base).
    fn di_void_ptr(&mut self) -> MetadataId {
        let bits = u64::from(self.ast.types.target().pointer_bytes) * 8;
        let f = Fields::default()
            .raw("tag", "DW_TAG_pointer_type")
            .nn("baseType", None)
            .u("size", bits);
        self.m.md_specialized("DIDerivedType", false, f.0)
    }

    /// The end of the prologue (clang `StartFunction`'s
    /// `EmitLocation(StartLoc)`); a thunk's body is artificial.
    pub(crate) fn di_prologue_end(&mut self, start: Loc, thunk: bool) {
        if !self.dbg_on() {
            return;
        }
        self.dbg_emit_location(start);
        if thunk {
            let sp = self.f.fir.dbg.as_deref().map(|d| d.sp);
            if let Some(sp) = sp {
                let l = self.di_location_at(0, 0, sp);
                self.f.fir.set_dbg_loc(Some(l));
            }
        }
    }

    /// clang `EmitDeclare` for variable `d` (`None`: the implicit `this`
    /// or VTT parameter, or a `pass_object_size` size) of type `ty`.
    fn di_declare(
        &mut self,
        d: Option<DeclId>,
        name: &str,
        ty: Option<MetadataId>,
        slot: VarSlot,
        this: bool,
    ) -> Option<MetadataId> {
        let ast = self.ast;
        let unwritten = d.is_none_or(|d| {
            let n = ast.decls.get(d);
            n.has(DeclFlags::IMPLICIT)
                || n.ctx().is_some_and(|c| ast.decls.get(c).has(DeclFlags::IMPLICIT))
        });
        let ty = ty?;
        let scope = self.f.fir.dbg.as_deref()?.scope();
        let (mut line, mut col, mut file) = (0, 0, None);
        if !unwritten && let Some(d) = d {
            let l = ast.decls.get(d).loc();
            (line, col) = self.di_line_col(l);
            file = Some(self.di_file(l));
        }
        let mut fl: Vec<&str> = Vec::new();
        if d.is_none_or(|d| ast.decls.get(d).has(DeclFlags::IMPLICIT)) {
            fl.push("DIFlagArtificial");
        }
        if this {
            fl.push("DIFlagObjectPointer");
        }
        let mut f = Fields::default();
        if !name.is_empty() {
            f = f.s("name", name);
        }
        if let Some(a) = slot.arg {
            f = f.u("arg", u64::from(a));
        }
        f = f
            .n("scope", Some(scope))
            .n("file", file)
            .u("line", u64::from(line))
            .n("type", Some(ty));
        f = f.flags("flags", &fl);
        let var = self.m.md_specialized("DILocalVariable", false, f.0);
        let expr = if slot.deref {
            self.m.md_specialized(
                "DIExpression",
                false,
                vec![("", MdField::Raw("DW_OP_deref".into()))],
            )
        } else {
            self.di.as_deref()?.empty_expr
        };
        let loc = self.di_location_at(line, col, scope);
        let opt = self.opts.opt_level > 0;
        if let Some(x) = self.f.fir.dbg.as_deref_mut() {
            x.tracked = true;
            if opt {
                x.vars.push(var);
            }
        }
        self.f.fir.declare(DbgRec { label: false, addr: slot.addr, var, expr, loc });
        Some(var)
    }

    /// clang `EmitAndRegisterVariableArrayDimensions`: each variable
    /// dimension of a variable length array, outermost first, gets an
    /// artificial `__vla_exprN` variable (a function-wide counter) on the
    /// `i64` slot holding it, which that dimension's `DISubrange` counts
    /// by; a dimension that folded to a constant counts by it
    /// ([dcl.array]; WP-1445).
    pub(crate) fn di_declare_vla_dims(&mut self, dims: &[(kiln_ast::ExprId, Result<V, i64>)]) {
        if !self.di_declaring() {
            return;
        }
        let size_t = QualType::unqual(self.ast.types.target().size_t);
        for &(size, dim) in dims {
            let count = match dim {
                Ok(slot) => {
                    let Some(x) = self.f.fir.dbg.as_deref_mut() else { return };
                    let name = format!("__vla_expr{}", x.vla_exprs);
                    x.vla_exprs += 1;
                    let ty = self.di_type(size_t);
                    let s = VarSlot { addr: slot, arg: None, deref: false };
                    match self.di_declare(None, &name, ty, s, false) {
                        Some(v) => MdField::Node(Some(v)),
                        None => continue,
                    }
                }
                Err(c) => MdField::Int(c),
            };
            if let Some(x) = self.di.as_deref_mut() {
                x.vla_counts.insert(size, count);
            }
        }
    }

    /// The method qualifiers of member function `m`, also through a
    /// function typedef (`typedef int CF() const;`).
    fn di_this_quals(&self, m: DeclId) -> u8 {
        self.di_fn_proto(m).map_or(0, |fp| fp.info().this_quals().bits())
    }

    fn di_declaring(&self) -> bool {
        self.dbg_on() && self.di_reduced() && self.f.fir.cur().is_some()
    }

    /// `this` (argument 1) at `addr`.
    pub(crate) fn di_declare_this(&mut self, addr: V) {
        if !self.di_declaring() {
            return;
        }
        let Some(rd) = self.f.this_class else { return };
        let quals = self.f.decl.map_or(0, |m| self.di_this_quals(m));
        let ty = self.di_this_ptr(rd, quals);
        let _ =
            self.di_declare(None, "this", ty, VarSlot { addr, arg: Some(1), deref: false }, true);
    }

    /// The VTT parameter (`void **`) at `addr`.
    pub(crate) fn di_declare_vtt(&mut self, addr: V, arg: u32) {
        if !self.di_declaring() {
            return;
        }
        let bits = u64::from(self.ast.types.target().pointer_bytes) * 8;
        let ptr = |cg: &mut Self, base: Option<MetadataId>| {
            let f = Fields::default()
                .raw("tag", "DW_TAG_pointer_type")
                .nn("baseType", base)
                .u("size", bits);
            cg.m.md_specialized("DIDerivedType", false, f.0)
        };
        let vp = ptr(self, None);
        let ty = ptr(self, Some(vp));
        let _ = self.di_declare(
            None,
            "vtt",
            Some(ty),
            VarSlot { addr, arg: Some(arg), deref: false },
            false,
        );
    }

    /// A `pass_object_size` parameter's implicit size (unnamed).
    pub(crate) fn di_declare_size(&mut self, addr: V, arg: u32) {
        if !self.di_declaring() {
            return;
        }
        let ty = self.di_type(QualType::unqual(self.ast.types.target().size_t));
        let _ =
            self.di_declare(None, "", ty, VarSlot { addr, arg: Some(arg), deref: false }, false);
    }

    /// Parameter or local variable `d` in `slot`.
    pub(crate) fn di_declare_var(&mut self, d: DeclId, slot: VarSlot) {
        if !self.di_declaring() || self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
        let Some(q) = self.ast.decls.get(d).ty() else { return };
        // clang `EmitAutoVarAlloca`'s `DI->setLocation(D.getLocation())`.
        if slot.arg.is_none()
            && let Some(x) = self.f.fir.dbg.as_deref_mut()
        {
            x.cur_loc = self.ast.decls.get(d).loc();
        }
        // clang `EmitDeclareOfAutoVariable`: a decomposition's storage is
        // no user variable; its bindings are.
        if slot.arg.is_none() && self.ast.decls.is_decomposition(d) {
            let by_ref = self.referee(q).is_some();
            self.di_declare_bindings(d, VarSlot { deref: slot.deref || by_ref, ..slot });
            return;
        }
        let name = self.di_ident(d);
        let ty = self.di_type(q);
        if ty.is_some() && slot.arg.is_none() {
            self.di_declare_anon_union_members(q, slot);
        }
        let _ = self.di_declare(Some(d), &name, ty, slot, false);
    }

    /// clang `EmitDeclare(const BindingDecl *)` for each binding of
    /// decomposition `d` stored in `slot` ([dcl.struct.bind]; WP-1445): a
    /// `DILocalVariable` on the decomposition's storage, at the member's
    /// byte offset (`DW_OP_plus_uconst`), a bit-field's bits extracted
    /// (`DW_OP_LLVM_extract_bits_*` after its storage unit's offset), or
    /// an array element's offset for a literal index. A tuple-like
    /// binding names its holding variable, which is declared on its own.
    fn di_declare_bindings(&mut self, d: DeclId, slot: VarSlot) {
        let ast = self.ast;
        let Some(list) = ast.decls.decomp_bindings(d) else { return };
        let Some(scope) = self.f.fir.dbg.as_deref().map(FirDbg::scope) else { return };
        let opt = self.opts.opt_level > 0;
        for b in ast.lists.iter::<DeclId>(list).collect::<Vec<_>>() {
            if self.has_fn_attr(b, AttrKind::NODEBUG) {
                continue;
            }
            let Some(e) = ast.decls.binding_expr(b) else { continue };
            let ek = ast.exprs.get(e).kind();
            if ek == kiln_ast::expr::ExprKind::DeclRefExpr || ast.decls.binding_var(b).is_some() {
                continue;
            }
            let loc = ast.decls.get(b).loc();
            let file = self.di_file(loc);
            let Some(bq) = ast.decls.get(b).ty() else { continue };
            let Some(ty) = self.di_type(bq) else { continue };
            let mut ops: Vec<String> = Vec::new();
            if slot.deref {
                ops.push("DW_OP_deref".into());
            }
            match ek {
                kiln_ast::expr::ExprKind::MemberExpr => {
                    let fd = ast.exprs.decl(e);
                    if ast.decls.get(fd).kind() == DeclKind::Field {
                        self.di_binding_field_ops(fd, bq, &mut ops);
                    }
                }
                kiln_ast::expr::ExprKind::ArraySubscriptExpr => {
                    let idx = ast.exprs.children(e, &ast.lists)[1];
                    if ast.exprs.get(idx).kind() == kiln_ast::expr::ExprKind::IntegerLiteral
                        && let Some(v) = ast.exprs.int_value(idx)
                        && v != 0
                    {
                        let size = self.size_align(bq).0;
                        ops.push(format!("DW_OP_plus_uconst, {}", v * size));
                    }
                }
                _ => {}
            }
            let (line, col) = self.di_line_col(loc);
            let name = self.di_ident(b);
            let mut f = Fields::default();
            if !name.is_empty() {
                f = f.s("name", &name);
            }
            f = f
                .n("scope", Some(scope))
                .n("file", Some(file))
                .u("line", u64::from(line))
                .n("type", Some(ty));
            let var = self.m.md_specialized("DILocalVariable", false, f.0);
            let expr = if ops.is_empty() {
                match self.di.as_deref() {
                    Some(x) => x.empty_expr,
                    None => return,
                }
            } else {
                let raw = ops.join(", ");
                self.m.md_specialized("DIExpression", false, vec![("", MdField::Raw(raw.into()))])
            };
            let l = self.di_location_at(line, col, scope);
            if let Some(x) = self.f.fir.dbg.as_deref_mut() {
                x.tracked = true;
                if opt {
                    x.vars.push(var);
                }
            }
            self.f.fir.declare(DbgRec { label: false, addr: slot.addr, var, expr, loc: l });
        }
    }

    /// The `DIExpression` operations of a binding to field `fd` (clang
    /// `EmitDeclare(const BindingDecl *)`): the field's byte offset in its
    /// own class, or for a bit-field its storage unit's offset and the
    /// bits to extract, at most the binding type's size `bq`.
    fn di_binding_field_ops(&mut self, fd: DeclId, bq: QualType, ops: &mut Vec<String>) {
        let ast = self.ast;
        let Some(def) = ast.decls.get(fd).ctx().and_then(|rd| self.class_def(rd)) else { return };
        if let Some(bf) = self.tys.record(def).bitfields.get(&fd).copied() {
            if bf.storage_offset != 0 {
                ops.push(format!("DW_OP_plus_uconst, {}", bf.storage_offset));
            }
            let op = if bf.is_signed {
                "DW_OP_LLVM_extract_bits_sext"
            } else {
                "DW_OP_LLVM_extract_bits_zext"
            };
            let size = u64::from(bf.size).min(self.size_align(bq).0 * 8);
            ops.push(format!("{op}, {}, {size}", bf.offset));
            return;
        }
        if let Ok(off) = self.field_byte_offset(fd)
            && off != 0
        {
            ops.push(format!("DW_OP_plus_uconst, {off}"));
        }
    }

    /// clang `EmitDeclare` for a block-scope anonymous union's unnamed
    /// variable: before the variable, an artificial `DILocalVariable` per
    /// member on the same storage, so a debugger finds `i` by name
    /// ([class.union.anon]/1; WP-1400). Unnamed members are skipped unless
    /// they are classes. The union's variable is implicit, so the members
    /// have no file and line 0, as it does.
    fn di_declare_anon_union_members(&mut self, q: QualType, slot: VarSlot) {
        let ast = self.ast;
        let Some(rd) = ast.types.as_record(ast.types.canonical(q)) else { return };
        let rd = ast.definition(rd).unwrap_or(rd);
        let Some(r) = ast.decls.record(rd) else { return };
        if r.tag != TagKind::Union || !r.defn.contains(DefinitionBits::ANONYMOUS) {
            return;
        }
        let Some(scope) = self.f.fir.dbg.as_deref().map(FirDbg::scope) else { return };
        let Some(empty) = self.di.as_deref().map(|x| x.empty_expr) else { return };
        let fields: Vec<DeclId> =
            ast.decls.members(rd).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        let opt = self.opts.opt_level > 0;
        for m in fields {
            let Some(fq) = ast.decls.get(m).ty() else { continue };
            let name = self.di_ident(m);
            let fty = self.di_type(fq);
            if name.is_empty() && ast.types.as_record(ast.types.canonical(fq)).is_none() {
                continue;
            }
            let Some(fty) = fty else { continue };
            let mut f = Fields::default();
            if !name.is_empty() {
                f = f.s("name", &name);
            }
            f = f.n("scope", Some(scope)).n("type", Some(fty));
            f = f.flags("flags", &["DIFlagArtificial"]);
            let var = self.m.md_specialized("DILocalVariable", false, f.0);
            let loc = self.di_location_at(0, 0, scope);
            if let Some(x) = self.f.fir.dbg.as_deref_mut() {
                x.tracked = true;
                if opt {
                    x.vars.push(var);
                }
            }
            let expr = if slot.deref {
                self.m.md_specialized(
                    "DIExpression",
                    false,
                    vec![("", MdField::Raw("DW_OP_deref".into()))],
                )
            } else {
                empty
            };
            self.f.fir.declare(DbgRec { label: false, addr: slot.addr, var, expr, loc });
        }
    }

    /// clang `EmitLabel`'s `CGDebugInfo::EmitLabel` for label `d`, whose
    /// block was just started: a `DILabel` and a `#dbg_label` at the
    /// block's end ([stmt.label]; WP-1445). When optimizing the label is
    /// kept in the subprogram's `retainedNodes` (`AlwaysPreserve`). `loc`
    /// is the defining statement's (clang moves a `LabelDecl` a `goto`
    /// created first to its definition).
    pub(crate) fn di_label(&mut self, d: DeclId, loc: Loc) {
        if !self.di_declaring() || self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
        let Some(scope) = self.f.fir.dbg.as_deref_mut().map(|x| {
            x.cur_loc = loc;
            x.scope()
        }) else {
            return;
        };
        let file = self.di_file(loc);
        let (line, col) = self.di_line_col(loc);
        let name = self.di_ident(d);
        let f = Fields::default()
            .n("scope", Some(scope))
            .s("name", &name)
            .n("file", Some(file))
            .u("line", u64::from(line));
        let label = self.m.md_specialized("DILabel", false, f.0);
        let l = self.di_location_at(line, col, scope);
        if self.opts.opt_level > 0
            && let Some(x) = self.f.fir.dbg.as_deref_mut()
        {
            x.tracked = true;
            x.labels.push(label);
        }
        let addr = V::C(self.m.const_bool(false));
        self.f.fir.declare(DbgRec { label: true, addr, var: label, expr: label, loc: l });
    }

    /// The subprogram's `retainedNodes` once it has a variable (clang
    /// `createParameterVariable` / `createAutoVariable` start the list;
    /// it keeps them with `AlwaysPreserve`, when optimizing).
    pub(crate) fn di_fn_finish(&mut self) {
        let Some(x) = self.f.fir.dbg.as_deref_mut() else { return };
        if !x.tracked {
            return;
        }
        let (sp, vars, labels) = (x.sp, std::mem::take(&mut x.vars), std::mem::take(&mut x.labels));
        let ops: Vec<Option<MetadataId>> = vars.into_iter().chain(labels).map(Some).collect();
        let t = self.m.md_tuple(&ops);
        let _ = self.m.md_set_field(sp, "retainedNodes", MdField::Node(Some(t)));
    }

    /// clang `EmitGlobalVariable(llvm::GlobalVariable *, const VarDecl *)`:
    /// the definition of variable `d` in global `g`.
    pub(crate) fn di_global_var(&mut self, d: DeclId, g: GlobalId) {
        if !self.di_reduced() || self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
        // clang's `DebugGlobalVariable` (spec/driver.md "Time trace").
        let _t = kiln_source::trace::Scope::with_detail("DebugGlobalVariable", || {
            self.qualified_name(d)
        });
        let ast = self.ast;
        let first = ast.first(d);
        let Some(dbg) = self.di.as_deref().map(|x| x.dbg) else { return };
        if let Some(&gve) = self.di.as_deref().and_then(|x| x.gves.get(&first)) {
            self.m.add_global_metadata(g, dbg, gve);
            return;
        }
        let n = ast.decls.get(d);
        let loc = n.loc();
        let Some(q) = n.ty() else { return };
        let Some(ty) = self.di_type(q) else { return };
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let name = self.di_ident(d);
        let rec = self.di_parent_record(first);
        let in_fn = n.ctx().is_some_and(|c| ast.decls.function(c).is_some());
        let gname = self.m.global(g).name().to_owned();
        let scope = match rec {
            Some(r) => match ast.decls.get(r).ctx() {
                Some(c) if ast.decls.get(c).kind() == DeclKind::Namespace => self.di_scope(r),
                _ => self.di.as_deref().map(|x| x.cu),
            },
            None => self.di_scope(d),
        };
        let decl = match rec.and_then(|r| ast.decls.get(r).ty()) {
            Some(rq) => match self.di_type(rq) {
                Some(rt) => self.di_static_member(first, rt),
                None => None,
            },
            None => None,
        };
        let tparams = self.di_template_params(d);
        // A namespace-scope structured binding's variable has no name;
        // clang leaves the field out (LLVM rejects `name: ""`).
        let mut f = Fields::default();
        if !name.is_empty() {
            f = f.s("name", &name);
        }
        if !in_fn && gname != name {
            f = f.s("linkageName", &gname);
        }
        f = f
            .n("scope", scope)
            .n("file", Some(file))
            .u("line", u64::from(line))
            .n("type", Some(ty));
        f = f.b("isLocal", self.m.global(g).linkage.is_local()).b("isDefinition", true);
        f = f.n("declaration", decl).n("templateParams", tparams);
        let var = self.m.md_specialized("DIGlobalVariable", true, f.0);
        let Some(expr) = self.di.as_deref().map(|x| x.empty_expr) else { return };
        let gf = Fields::default().n("var", Some(var)).n("expr", Some(expr));
        let gve = self.m.md_specialized("DIGlobalVariableExpression", false, gf.0);
        self.m.add_global_metadata(g, dbg, gve);
        if let Some(x) = self.di.as_deref_mut() {
            x.gves.insert(first, gve);
            x.globals.push(gve);
        }
    }

    /// clang `EmitDeclRefExprDbgValue` -> `EmitGlobalVariable(const
    /// ValueDecl *, const APValue &)`: `d` (an enumerator, or a variable
    /// read as its constant `c`) gets its type; a namespace-scope variable
    /// a constant global variable expression; a static data member keeps
    /// its class (WP-814 follow-up: clang skips variables
    /// `DeclMustBeEmitted`, approximated by the existing definition).
    pub(crate) fn di_const_ref(&mut self, d: DeclId, c: Option<kiln_ast::ConstValueId>) {
        if !self.dbg_on() || !self.di_reduced() || self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
        let ast = self.ast;
        let n = ast.decls.get(d);
        // clang's `tryEmitAsConstant` makes the reference for a variable
        // or an enumerator only: a folded non-static member (`p.a` of a
        // template parameter object) or the parameter object itself
        // describes nothing, so its class is no retained type.
        if !matches!(n.kind(), DeclKind::Var | DeclKind::EnumConstant) {
            return;
        }
        let Some(q) = n.ty() else { return };
        let ty = self.di_type(q);
        if n.kind() == DeclKind::EnumConstant {
            if let Some(ed) = n.ctx().and_then(|e| ast.decls.get(e).ty()) {
                let _ = self.di_type(ed);
            }
            return;
        }
        if n.ctx().is_some_and(|x| ast.decls.function(x).is_some()) {
            return;
        }
        let first = ast.first(d);
        if let Some(r) = self.di_parent_record(first) {
            if let Some(t) = ast.decls.get(r).ty().and_then(|rq| self.di_type(rq))
                && let Some(x) = self.di.as_deref_mut()
                && !x.retained.contains(&t)
            {
                x.retained.push(t);
            }
            return;
        }
        if self.di.as_deref().is_none_or(|x| x.gves.contains_key(&first)) {
            return;
        }
        let scope = self.di_scope(d);
        let loc = ast.decls.get(first).loc();
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let mut ops = String::new();
        if self.size_align(q).0 <= 8 {
            let v = c.and_then(|c| match ast.consts.get(c) {
                kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Int {
                    bits,
                    signed,
                    ..
                }) => {
                    if signed {
                        bits.to_i128().and_then(|v| i64::try_from(v).ok()).map(|v| v as u64)
                    } else {
                        bits.to_u128().and_then(|v| u64::try_from(v).ok())
                    }
                }
                kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Bool {
                    value,
                    ..
                }) => Some(u64::from(value)),
                kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Float {
                    bits,
                    ..
                }) => u64::try_from(bits).ok(),
                _ => None,
            });
            if let Some(v) = v {
                ops = format!("DW_OP_constu, {v}, DW_OP_stack_value");
            }
        }
        let tparams = self.di_template_params(d);
        let name = self.di_ident(d);
        let f = Fields::default()
            .s("name", &name)
            .n("scope", scope)
            .n("file", Some(file))
            .u("line", u64::from(line));
        let f =
            f.n("type", ty).b("isLocal", true).b("isDefinition", true).n("templateParams", tparams);
        let var = self.m.md_specialized("DIGlobalVariable", true, f.0);
        let expr = if ops.is_empty() {
            self.di.as_deref().map(|x| x.empty_expr)
        } else {
            Some(self.m.md_specialized("DIExpression", false, vec![("", MdField::Raw(ops.into()))]))
        };
        let gf = Fields::default().n("var", Some(var)).n("expr", expr);
        let gve = self.m.md_specialized("DIGlobalVariableExpression", false, gf.0);
        if let Some(x) = self.di.as_deref_mut() {
            x.gves.insert(first, gve);
            x.globals.push(gve);
        }
    }

    /// The end of a function body (clang `Body->getEndLoc()`).
    pub(crate) fn di_body_end(&self, body: Option<StmtId>) -> Loc {
        let ast = self.ast;
        match body {
            Some(b) if ast.stmts.get(b).kind() == kiln_ast::stmt::StmtKind::CompoundStmt => {
                ast.stmts.rbrace(b)
            }
            // A coroutine's `CoroutineBodyStmt` ends where its body does.
            Some(b) if ast.stmts.get(b).kind() == kiln_ast::stmt::StmtKind::CoroutineBodyStmt => {
                self.di_body_end(Some(ast.stmts.coroutine_body(b).body))
            }
            _ => Loc::INVALID,
        }
    }

    /// clang `FinishFunction` before the prologue's cleanups are popped:
    /// the location of the cleanups and of the return.
    pub(crate) fn di_finish_function(&mut self, end: Loc) {
        let prologue = self.f.cl.prologue;
        let has_cleanups = self.f.cl.depth() != prologue;
        let only_markers = has_cleanups
            && self.f.cl.stack[prologue..].iter().all(crate::cleanup::Scope::is_lifetime_marker);
        let ret_used = self.f.ret_block.is_some_and(|rb| self.f.fir.block_uses(rb) > 0);
        let Some(x) = self.f.fir.dbg.as_deref_mut() else { return };
        x.ret_dbg = !has_cleanups || only_markers;
        let simple = x.simple_returns > 0 && x.simple_returns == x.returns && !ret_used;
        let last = x.last_stop;
        if simple {
            self.dbg_emit_location(last);
        } else {
            self.dbg_emit_location(end);
        }
        if has_cleanups {
            if simple {
                self.dbg_emit_location(end);
            } else {
                let _ = self.dbg_apply_default_artificial(end);
            }
        }
    }
}
