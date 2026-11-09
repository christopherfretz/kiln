//! Function subprograms and variables (clang `emitFunctionStart`,
//! `EmitDeclare`, `EmitGlobalVariable`).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::types::TypeView;
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
            if reduced && fname != name {
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
                let noreturn = self.has_fn_attr(d, AttrKind::NORETURN)
                    || matches!(n.ty().map(|q| ast.types.view(q.ty())), Some(TypeView::Function(fp)) if fp.info().noreturn());
                if noreturn {
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
                match n.ty().and_then(|q| self.di_type(q)) {
                    Some(t) => t,
                    None => fake(self),
                }
            };
            decl = if reduced && method { self.di_method_decl(d) } else { None };
        }
        if self.opts.opt_level > 0 && self.opts.debug.dwarf_version.unwrap_or(5) >= 5 {
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
    ) {
        let ast = self.ast;
        let unwritten = d.is_none_or(|d| {
            let n = ast.decls.get(d);
            n.has(DeclFlags::IMPLICIT)
                || n.ctx().is_some_and(|c| ast.decls.get(c).has(DeclFlags::IMPLICIT))
        });
        let Some(ty) = ty else { return };
        let scope = match self.f.fir.dbg.as_deref() {
            Some(x) => x.scope(),
            None => return,
        };
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
            match self.di.as_deref() {
                Some(x) => x.empty_expr,
                None => return,
            }
        };
        let loc = self.di_location_at(line, col, scope);
        let opt = self.opts.opt_level > 0;
        if let Some(x) = self.f.fir.dbg.as_deref_mut() {
            x.tracked = true;
            if opt {
                x.vars.push(var);
            }
        }
        self.f.fir.declare(DbgRec { addr: slot.addr, var, expr, loc });
    }

    /// The method qualifiers of member function `m`.
    fn di_this_quals(&self, m: DeclId) -> u8 {
        match self.ast.decls.get(m).ty().map(|q| self.ast.types.view(q.ty())) {
            Some(TypeView::Function(fp)) => fp.info().this_quals().bits(),
            _ => 0,
        }
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
        self.di_declare(
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
        let name = self.di_ident(d);
        let ty = self.di_type(q);
        self.di_declare(Some(d), &name, ty, slot, false);
    }

    /// The subprogram's `retainedNodes` once it has a variable (clang
    /// `createParameterVariable` / `createAutoVariable` start the list;
    /// it keeps them with `AlwaysPreserve`, when optimizing).
    pub(crate) fn di_fn_finish(&mut self) {
        let Some(x) = self.f.fir.dbg.as_deref_mut() else { return };
        if !x.tracked {
            return;
        }
        let (sp, vars) = (x.sp, std::mem::take(&mut x.vars));
        let ops: Vec<Option<MetadataId>> = vars.into_iter().map(Some).collect();
        let t = self.m.md_tuple(&ops);
        let _ = self.m.md_set_field(sp, "retainedNodes", MdField::Node(Some(t)));
    }

    /// clang `EmitGlobalVariable(llvm::GlobalVariable *, const VarDecl *)`:
    /// the definition of variable `d` in global `g`.
    pub(crate) fn di_global_var(&mut self, d: DeclId, g: GlobalId) {
        if !self.di_reduced() || self.has_fn_attr(d, AttrKind::NODEBUG) {
            return;
        }
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
