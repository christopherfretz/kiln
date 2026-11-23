//! The signature area (K-3e; mortar's ruling of 11-17 06:29 ET): a
//! function template specialization NAMED in a hashed definition (a
//! callee, an address taken, a default argument, a member pointer) hashes
//! by ENTITY, not by its mangled name. The mangled name carries the
//! template's dependent signature as the unit's first equivalent spelling
//! wrote it (kiln keeps the first expression of each [temp.over.link]
//! equivalence class for printing, like clang), so two units that spell a
//! dependent name of a redeclared or equivalent signature differently
//! give the same entity two symbols (fmt's chrono test).
//!
//! The item: the template's qualified name, the template arguments (the
//! canonical list's `<template-args>` encoding: non-dependent, so its
//! spelling is the value's), then the template's signature digest:
//! its template parameter list (kind, pack, a non-type parameter's type,
//! the type-constraint, the requires-clause), the pattern's canonical
//! function type and its trailing requires-clause, every name by entity
//! (template parameters by depth and index, function parameters by
//! position, dependent names by qualifier and spelling, an equivalence
//! class's expression in its canonical form). The digest is memoized per
//! template (first declaration), so a redeclaration cannot change it.
//!
//! A signature holding a node this area does not cover takes the short
//! form (no signature digest); then the row is `PARTIAL` when the template
//! has another function template of the same name and template parameter
//! count in its scope (the ruling's fallback). The dependent types and
//! expressions are hashed the same way wherever they appear (a generic
//! lambda's pattern).

use kiln_ast::decl::{DeclKind, ParamData};
use kiln_ast::expr::ExprKind as K;
use kiln_ast::ids::{ArgListId, ListId, NnsId, ProfileId, TemplateParamListId};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::types::{ArgView, ExcSpec, ExprOperand, Nns, TemplateName, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, RawId};

use super::{Area, Callee, DefHasher, Item, Site, T_NONE, T_TU_PLACEHOLDER, Work, fnv1a_128_bytes};

/// A dependent type: qualifiers, kind, then its layout.
const T_DEP_TYPE: u8 = 0xB7;
/// A template parameter: kind byte, `u32` depth, `u32` index, pack byte.
const T_TPARM: u8 = 0xB8;
/// A function parameter by position: `u32` index, pack, scope depth, type.
const T_PARM_POS: u8 = 0xB9;
/// A template by entity: its scope path and name.
const T_TEMPLATE: u8 = 0xBA;
/// A function template specialization by entity (the item's digest).
const T_CALLEE: u8 = 0xBB;
/// A nested-name-specifier (canonical).
const T_NNS: u8 = 0xBC;

/// Deeper dependent types than this are not covered (no native stack
/// overflow on pathological types).
const MAX_TYPE_DEPTH: u32 = 48;

impl DefHasher<'_> {
    /// Whether canonical type `c` must take the structural path (it is
    /// dependent or instantiation-dependent: its encoding would print an
    /// equivalence class's first spelling).
    pub(super) fn is_dep_type(&self, c: QualType) -> bool {
        self.ast.types.qflags(c).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
    }

    /// Function template specialization `d` by entity (module docs).
    pub(super) fn template_callee(&mut self, d: DeclId) {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&c) = self.callees.get(&first) {
            return self.write_callee(c);
        }
        let Some((_, s)) = ast.spec_info(first) else { return self.byte(T_TU_PLACEHOLDER) };
        let tmpl = ast.first(s.template);
        let args = ast.types.canonical_args(s.args);
        let start = self.out.len();
        let outer = std::mem::replace(&mut self.partial, false);
        let outer_system = std::mem::replace(&mut self.refs.named_system, false);
        self.byte(T_CALLEE);
        self.byte(ast.decls.get(first).kind() as u8);
        self.template_entity(tmpl);
        let enc = crate::mangle::mangle_template_args(ast, self.refs.anon_mut(), args);
        self.str(&enc);
        let full = self.signature(tmpl);
        match full {
            Some(h) => {
                self.byte(1);
                self.out.extend_from_slice(&h.to_le_bytes());
            }
            None => self.byte(0),
        }
        self.stats.callee_full += u64::from(full.is_some());
        self.stats.callee_short += u64::from(full.is_none());
        let h = fnv1a_128_bytes(&self.out[start..]);
        self.out.truncate(start);
        let overloaded = full.is_none() && self.has_overload(tmpl);
        // The item's own skips (memoized with it) join the definition's.
        let partial = std::mem::replace(&mut self.partial, outer);
        // WP-1627: the template and arguments are written as strings, not
        // through the memo, so they are asked here.
        let system = std::mem::replace(&mut self.refs.named_system, outer_system)
            || self.refs.template_item_system(tmpl, s.args);
        let c = Callee { hash: h, partial, overloaded, system };
        self.callees.insert(first, c);
        self.write_callee(c);
    }

    fn write_callee(&mut self, c: Callee) {
        self.byte(T_CALLEE);
        self.out.extend_from_slice(&c.hash.to_le_bytes());
        self.refs.named_system |= c.system;
        self.join_partial(c.partial);
        if c.overloaded {
            self.partial(Area::Dependent, Site::Item(Item::ShortCallee));
        }
    }

    /// The signature digest of function template `tmpl` (first
    /// declaration), `None` when it holds a node this area does not
    /// cover. Memoized per template.
    fn signature(&mut self, tmpl: DeclId) -> Option<u128> {
        let ast = self.ast;
        if let Some(&(r, system)) = self.sigs.get(&tmpl) {
            self.refs.named_system |= system;
            return r;
        }
        // A signature naming its own template meets this entry.
        self.sigs.insert(tmpl, (None, false));
        let info = ast.template_info(tmpl)?;
        let pattern = info.pattern?;
        self.stats.signatures += 1;
        let start = self.out.len();
        let outer_system = std::mem::replace(&mut self.refs.named_system, false);
        let uncovered = std::mem::replace(&mut self.sig_uncovered, false);
        self.sig += 1;
        self.positional += 1;
        self.template_params(info.params);
        match ast.decls.get(pattern).ty() {
            Some(t) => self.ty(t),
            None => self.byte(T_NONE),
        }
        let req = ast.redecls_of(pattern).find_map(|r| ast.templates.trailing_requires.get(r));
        self.opt_expr_now(req.copied());
        self.sig -= 1;
        self.positional -= 1;
        let covered = !std::mem::replace(&mut self.sig_uncovered, uncovered);
        let r = covered.then(|| fnv1a_128_bytes(&self.out[start..]));
        self.out.truncate(start);
        let system = std::mem::replace(&mut self.refs.named_system, outer_system);
        self.refs.named_system |= system;
        self.sigs.insert(tmpl, (r, system));
        r
    }

    /// A template parameter list: per parameter its kind, pack, a
    /// non-type parameter's type, a template template parameter's own
    /// list, the type-constraint; then the requires-clause. Default
    /// arguments are not part of a signature ([temp.over.link]).
    fn template_params(&mut self, pl: TemplateParamListId) {
        let ast = self.ast;
        let pl = ast.templates.param_list(pl);
        self.count(ast.lists.len(pl.params));
        for p in ast.lists.iter::<DeclId>(pl.params) {
            let n = ast.decls.get(p);
            let parm = ast.template_parm(p).copied();
            self.byte(n.kind() as u8);
            self.byte(u8::from(parm.is_some_and(|x| x.pack)));
            if n.kind() == DeclKind::NonTypeTemplateParm {
                match n.ty() {
                    Some(t) => self.ty(t),
                    None => self.byte(T_NONE),
                }
            }
            match parm.and_then(|x| x.params) {
                Some(inner) => self.template_params(inner),
                None => self.byte(T_NONE),
            }
            self.opt_expr_now(parm.and_then(|x| x.constraint));
        }
        self.opt_expr_now(pl.requires);
    }

    /// Walks an optional expression now (a nested walk).
    pub(super) fn opt_expr_now(&mut self, e: Option<ExprId>) {
        let base = self.stack.len();
        self.stack.push(Work::OptExpr(e));
        self.run_nested(base);
    }

    /// Whether another function template could be named by the same
    /// short form as `tmpl` (WP-1626(b)): declared in `tmpl`'s scope with
    /// its name (the fallback's condition, read at the template's scope
    /// rather than at the call), with the same template parameter kinds
    /// in order ([`Self::tparm_shape`]: a specialization of a template of
    /// other kinds has other template arguments, so another short form),
    /// and a function parameter count range that overlaps `tmpl`'s (the
    /// call's argument count is in `tmpl`'s range, and a template that
    /// cannot take it is never chosen for it).
    fn has_overload(&self, tmpl: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(tmpl);
        let own = (self.tparm_shape(tmpl), self.arity(tmpl));
        let Some(ctx) = n.ctx() else { return false };
        ast.redecls_of(ctx).any(|c| {
            ast.members(c).any(|m| {
                let mn = ast.decls.get(m);
                mn.kind() == DeclKind::FunctionTemplate
                    && mn.name() == n.name()
                    && ast.first(m) != tmpl
                    && Self::shapes_match(own.0.as_deref(), self.tparm_shape(m).as_deref())
                    && Self::ranges_meet(own.1, self.arity(m))
            })
        })
    }

    /// The template parameter kinds of function template `t` in order:
    /// per parameter (declaration kind, pack, the class of a constant
    /// parameter's type: 1 integral or enumeration, 2 floating, 3 pointer
    /// or member pointer, 4 reference, 5 class, 0 any (dependent,
    /// placeholder, other)). `None` when `t` has no parameter list.
    fn tparm_shape(&self, t: DeclId) -> Option<Vec<(u8, bool, u8)>> {
        use kiln_ast::types::TypeKind as T;
        let ast = self.ast;
        let info = ast.template_info(t)?;
        let pl = ast.templates.param_list(info.params);
        let shape = ast.lists.iter::<DeclId>(pl.params).map(|p| {
            let node = ast.decls.get(p);
            let pack = ast.template_parm(p).is_some_and(|x| x.pack);
            let class = match node.ty() {
                Some(q) if node.kind() == DeclKind::NonTypeTemplateParm => {
                    match ast.types.canonical_kind(q) {
                        T::Builtin if ast.types.is_integral(q) => 1,
                        T::BitInt | T::Enum => 1,
                        T::Builtin if ast.types.is_floating(q) => 2,
                        T::Pointer | T::MemberPointer => 3,
                        T::LValueRef | T::RValueRef => 4,
                        T::Record => 5,
                        _ => 0,
                    }
                }
                _ => 0,
            };
            (node.kind() as u8, pack, class)
        });
        Some(shape.collect())
    }

    /// Whether two templates' parameter kinds could give one argument
    /// list: equal lengths and kinds, constant classes equal or one of
    /// them any; a pack on either side, or an unknown list, matches.
    fn shapes_match(a: Option<&[(u8, bool, u8)]>, b: Option<&[(u8, bool, u8)]>) -> bool {
        let (Some(a), Some(b)) = (a, b) else { return true };
        if a.iter().chain(b).any(|p| p.1) {
            return true;
        }
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| x.0 == y.0 && (x.2 == y.2 || x.2 == 0 || y.2 == 0))
    }

    /// The argument counts a call of function template `t` can have:
    /// (fewest, most; `None` without a bound). The fewest is the least over
    /// the pattern's declarations (a later one may add default
    /// arguments), one less for a class member (an explicit object
    /// parameter is not an argument of a member call); a parameter pack
    /// or `...` has no bound.
    fn arity(&self, t: DeclId) -> (usize, Option<usize>) {
        let ast = self.ast;
        let Some(pattern) = ast.template_info(t).and_then(|i| i.pattern) else {
            return (0, None);
        };
        let Some(p) = ast.decls.get(pattern).ty().and_then(|q| ast.types.proto(q)) else {
            return (0, None);
        };
        let params = ast.types.params(p);
        let packs = params
            .iter()
            .any(|&q| ast.types.canonical_kind(q) == kiln_ast::types::TypeKind::PackExpansion);
        let max = (!p.variadic() && !packs).then_some(params.len());
        let fewest =
            ast.redecls_of(pattern).map(|r| ast.decls.min_args(r).unwrap_or(0)).min().unwrap_or(0);
        let member = ast.decls.get(t).ctx().is_some_and(|c| {
            matches!(
                ast.decls.get(c).kind(),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
            )
        });
        (fewest.saturating_sub(usize::from(member)), max)
    }

    /// Whether two argument count ranges share a count.
    fn ranges_meet(a: (usize, Option<usize>), b: (usize, Option<usize>)) -> bool {
        a.0 <= b.1.unwrap_or(usize::MAX) && b.0 <= a.1.unwrap_or(usize::MAX)
    }

    /// A template (class, alias, variable, function or concept template,
    /// or a template template parameter) by entity.
    fn template_entity(&mut self, d: DeclId) {
        let ast = self.ast;
        let d = ast.first(d);
        let n = ast.decls.get(d);
        if n.kind() == DeclKind::TemplateTemplateParm {
            return self.tparm(d);
        }
        self.byte(T_TEMPLATE);
        self.byte(n.kind() as u8);
        self.scope_path(n.ctx());
        self.decl_name(n.name());
    }

    /// The enclosing scopes of an entity, innermost last: named
    /// namespaces by name, a class by its type item (which names its own
    /// scope). A function scope is not covered.
    fn scope_path(&mut self, mut ctx: Option<DeclId>) {
        let ast = self.ast;
        let mut names: [Option<DeclId>; 16] = [None; 16];
        let mut n = 0;
        let mut class = None;
        while let Some(c) = ctx {
            let node = ast.decls.get(c);
            match node.kind() {
                DeclKind::Namespace if n < names.len() => {
                    names[n] = Some(c);
                    n += 1;
                }
                DeclKind::LinkageSpec | DeclKind::Export => {}
                DeclKind::TranslationUnit => break,
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                    if !super::is_templated_def(ast, c) =>
                {
                    class = Some(c);
                    break;
                }
                k => {
                    self.partial(Area::Dependent, Site::Decl(k));
                    break;
                }
            }
            ctx = node.ctx();
        }
        match class {
            Some(c) => self.owner_of(c),
            None => self.byte(T_NONE),
        }
        self.count(n);
        for &ns in names[..n].iter().rev().flatten() {
            self.name(ns);
        }
    }

    /// Class `c` as a type item (a scope of a template).
    pub(super) fn owner_of(&mut self, c: DeclId) {
        match self.refs.tag(c, &mut self.stats) {
            Some(h) => {
                self.byte(super::T_TYPE);
                self.out.extend_from_slice(&h.to_le_bytes());
            }
            None => self.local_tag(c),
        }
    }

    /// A declaration name (identifier, operator, conversion type, ...).
    pub(super) fn decl_name(&mut self, dn: DeclName) {
        let ast = self.ast;
        match ast.names.view(dn) {
            DeclNameView::Empty => self.byte(0),
            DeclNameView::Identifier(s) => {
                self.byte(1);
                self.str(ast.symbols().as_str(s));
            }
            DeclNameView::Operator(op) => {
                self.byte(2);
                self.byte(op as u8);
            }
            DeclNameView::Conversion(q) => {
                self.byte(3);
                self.ty(q);
            }
            DeclNameView::Constructor(_) => self.byte(4),
            DeclNameView::Destructor(_) => self.byte(5),
            DeclNameView::LiteralOperator(s) => {
                self.byte(6);
                self.str(ast.symbols().as_str(s));
            }
            DeclNameView::DeductionGuide(_) => {
                self.partial(Area::Dependent, Site::Item(Item::DependentType));
            }
        }
    }

    /// A template parameter declaration by position.
    pub(super) fn tparm(&mut self, d: DeclId) {
        let ast = self.ast;
        let Some(p) = ast.template_parm(d) else {
            return self.partial(Area::Dependent, Site::Decl(ast.decls.get(d).kind()));
        };
        self.byte(T_TPARM);
        self.byte(ast.decls.get(d).kind() as u8);
        self.u32(u32::from(p.depth));
        self.u32(u32::from(p.index));
        self.byte(u8::from(p.pack));
    }

    /// A function parameter of an equivalence class's expression by
    /// position (the class's first expression may name another
    /// declaration's parameters).
    pub(super) fn parm_pos(&mut self, d: DeclId) {
        let ast = self.ast;
        let (index, pack, depth) = ast.decls.param(d).map_or((0, false, 0), |p| {
            (u32::from(p.index), p.flags & ParamData::PACK != 0, p.scope_depth())
        });
        let in_requires = ast
            .decls
            .get(d)
            .ctx()
            .is_some_and(|c| ast.decls.get(c).kind() == DeclKind::RequiresExprBody);
        self.byte(T_PARM_POS);
        self.u32(index);
        self.byte(u8::from(pack) | u8::from(in_requires) << 1);
        self.u32(depth);
        match ast.decls.get(d).ty() {
            Some(t) => self.ty(t),
            None => self.byte(T_NONE),
        }
    }

    /// The canonical form of an equivalence class: its first expression,
    /// walked with function parameters by position.
    pub(super) fn profile(&mut self, p: ProfileId) {
        match self.ast.types.profile_expr(p) {
            Some(e) => {
                self.positional += 1;
                self.opt_expr_now(Some(e));
                self.positional -= 1;
            }
            None => self.partial(Area::Dependent, Site::Item(Item::DependentType)),
        }
    }

    fn operand(&mut self, o: ExprOperand) {
        match o {
            ExprOperand::Profile(p) => self.profile(p),
            ExprOperand::Expr(e) => self.opt_expr_now(Some(e)),
        }
    }

    /// A nested-name-specifier, canonical: a namespace by its path, a type
    /// by its type item, a dependent identifier by its prefix and name.
    pub(super) fn nns(&mut self, n: Option<NnsId>) {
        let ast = self.ast;
        let Some(n) = n else { return self.byte(T_NONE) };
        self.byte(T_NNS);
        match ast.types.nns_view(ast.types.canonical_nns(n)) {
            Nns::Global => self.byte(1),
            Nns::Namespace { ns, .. } | Nns::NamespaceAlias { ns, .. } => {
                self.byte(2);
                self.scope_path(Some(ns));
            }
            Nns::Type { ty, .. } => {
                self.byte(3);
                self.ty(ty);
            }
            Nns::Identifier { prefix, name } => {
                self.byte(4);
                self.nns(prefix);
                self.str(ast.symbols().as_str(name));
            }
            Nns::Splice(_) => self.partial(Area::Dependent, Site::Item(Item::DependentType)),
        }
    }

    /// Template arguments that may be dependent, canonical.
    pub(super) fn dep_args(&mut self, list: Option<ArgListId>) {
        let ast = self.ast;
        let Some(list) = list else { return self.byte(T_NONE) };
        let list = ast.types.canonical_args(list);
        let args = ast.types.args(list);
        self.count(args.len());
        for a in args {
            match a.view() {
                ArgView::Null => self.byte(T_NONE),
                ArgView::Type(q) => {
                    self.byte(1);
                    self.ty(q);
                }
                ArgView::Value { ty, value } => {
                    self.byte(2);
                    self.ty(ty);
                    match ast.consts.scalar(value) {
                        Some(kiln_ast::consts::Scalar::Int { bits, .. }) => {
                            self.count(bits.words().len());
                            for &w in bits.words() {
                                self.u64(w);
                            }
                        }
                        Some(kiln_ast::consts::Scalar::Bool { value, .. }) => {
                            self.count(1);
                            self.u64(u64::from(value));
                        }
                        _ => self.partial(Area::Dependent, Site::Item(Item::DependentType)),
                    }
                }
                ArgView::Template(t) => {
                    self.byte(3);
                    self.template_name(t);
                }
                ArgView::TemplateExpansion { name, num_expansions } => {
                    self.byte(4);
                    self.template_name(name);
                    self.u32(num_expansions.map_or(u32::MAX, |x| x));
                }
                ArgView::Profile(p) | ArgView::Expr { profile: Some(p), .. } => {
                    self.byte(5);
                    self.profile(p);
                }
                ArgView::Expr { expr, profile: None } => {
                    self.byte(5);
                    self.opt_expr_now(Some(expr));
                }
                ArgView::Pack(p) => {
                    self.byte(6);
                    self.dep_args(Some(p));
                }
            }
        }
    }

    fn template_name(&mut self, t: kiln_ast::ids::TemplateNameId) {
        let ast = self.ast;
        match ast.types.template_name_view(ast.types.canonical_template_name(t)) {
            TemplateName::Template(d) => self.template_entity(d),
            TemplateName::Qualified { underlying, .. } => self.template_name(underlying),
            TemplateName::DependentIdentifier { nns, name } => {
                self.byte(1);
                self.nns(Some(nns));
                self.str(ast.symbols().as_str(name));
            }
            TemplateName::DependentOperator { nns, op } => {
                self.byte(2);
                self.nns(Some(nns));
                self.byte(op);
            }
            TemplateName::TemplateTemplateParm { depth, index, pack, .. }
            | TemplateName::CanonTemplateTemplateParm { depth, index, pack } => {
                self.byte(T_TPARM);
                self.byte(DeclKind::TemplateTemplateParm as u8);
                self.u32(depth);
                self.u32(index);
                self.byte(u8::from(pack));
            }
            #[allow(unreachable_patterns, reason = "other template name forms")]
            _ => self.partial(Area::Dependent, Site::Item(Item::DependentType)),
        }
    }

    /// Dependent canonical type `c` by structure (module docs); what it
    /// holds that is not dependent goes back through [`Self::ty`].
    #[allow(clippy::too_many_lines, reason = "one arm per dependent type kind")]
    pub(super) fn dep_type(&mut self, c: QualType) {
        let ast = self.ast;
        if self.type_depth >= MAX_TYPE_DEPTH {
            return self.partial(Area::Dependent, Site::Item(Item::DependentType));
        }
        self.type_depth += 1;
        self.byte(T_DEP_TYPE);
        self.byte(c.quals().bits());
        self.byte(ast.types.kind(c.ty()) as u8);
        match ast.types.canonical_view(c) {
            TypeView::CanonTemplateTypeParm { depth, index, pack } => {
                self.u32(depth);
                self.u32(index);
                self.byte(u8::from(pack));
            }
            TypeView::Pointer(p)
            | TypeView::LValueRef(p)
            | TypeView::RValueRef(p)
            | TypeView::IncompleteArray(p)
            | TypeView::Complex(p)
            | TypeView::Atomic(p) => self.ty(p),
            TypeView::MemberPointer { pointee, class } => {
                self.ty(pointee);
                self.ty(QualType::unqual(class));
            }
            TypeView::ConstantArray(a) => {
                self.u64(a.size);
                self.ty(a.elem);
            }
            TypeView::DependentSizedArray { elem, size }
            | TypeView::DependentAddrSpace { pointee: elem, space: size } => {
                self.ty(elem);
                self.operand(size);
            }
            TypeView::Function(p) => {
                self.ty(p.ret());
                let params = ast.types.params(&p);
                self.count(params.len());
                for &q in params {
                    self.ty(q);
                }
                // Not `noreturn`: as the declarator (WP-1637).
                self.u32(p.info().with_trailing_return(false).with_noreturn(false).bits());
                match ast.types.exc_spec(&p) {
                    // A dependent `noexcept(e)`: its equivalence class's
                    // expression, like any other.
                    ExcSpec::DependentNoexcept(c)
                    | ExcSpec::NoexceptExpr {
                        value: kiln_ast::types::NoexceptValue::Dependent(c),
                        ..
                    } => {
                        self.byte(3);
                        self.profile(c);
                    }
                    ExcSpec::Deferred(d) => self.deferred_noexcept(d),
                    e => self.byte(u8::from(e.is_nothrow())),
                }
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                self.template_name(name);
                self.dep_args(Some(args));
            }
            TypeView::DependentName { nns, name, .. } => {
                self.nns(Some(nns));
                self.str(ast.symbols().as_str(name));
            }
            TypeView::DependentTemplateSpecialization { nns, name, args, .. } => {
                self.nns(Some(nns));
                self.str(ast.symbols().as_str(name));
                self.dep_args(Some(args));
            }
            TypeView::PackExpansion { pattern, num_expansions } => {
                self.u32(num_expansions.map_or(u32::MAX, |x| x));
                self.ty(pattern);
            }
            TypeView::PackIndexing { pattern, index, .. } => {
                self.ty(pattern);
                self.operand(index);
            }
            TypeView::DependentDecltype(p) => self.profile(p),
            TypeView::Decltype { expr, .. } => {
                self.positional += 1;
                self.opt_expr_now(Some(expr));
                self.positional -= 1;
            }
            TypeView::UnaryTransform { kind, base, .. } => {
                self.byte(kind as u8);
                self.ty(base);
            }
            TypeView::Auto { keyword, deduced: None, concept, args, pack } => {
                self.byte(keyword as u8);
                self.byte(u8::from(pack));
                match concept {
                    Some(d) => self.template_entity(d),
                    None => self.byte(T_NONE),
                }
                self.dep_args(args);
            }
            // A placeholder deduced to a dependent type (a generic
            // lambda's `auto` return): the deduced type is what is written.
            TypeView::Auto { keyword, deduced: Some(q), .. } => {
                self.byte(keyword as u8);
                self.ty(q);
            }
            // The type of an expression whose type is not known yet: the
            // expression itself is hashed by structure.
            TypeView::Builtin(b) => self.byte(b as u8),
            TypeView::DependentBitInt { signed, width } => {
                self.byte(u8::from(signed));
                self.profile(width);
            }
            _ => self.partial(Area::Dependent, Site::Item(Item::DependentType)),
        }
        self.type_depth -= 1;
    }

    /// The exception specification of `d` that sema has not computed yet
    /// (`ExcSpec::Deferred(d)`: a member of a class template
    /// specialization, or a function template specialization, still
    /// waiting for its `noexcept` to be instantiated; `d` is the pattern
    /// it waits on, or itself) by structure, from that pattern (WP-1626):
    /// a dependent `noexcept(e)` as the equivalence class's expression,
    /// like any other, a known one by its value. Nothing is instantiated:
    /// the hasher only reads the AST. An implicit or defaulted member (no
    /// pattern), or a dynamic specification, stays a skip.
    fn deferred_noexcept(&mut self, d: DeclId) {
        use kiln_ast::types::NoexceptValue;
        let ast = self.ast;
        let spec_of = |d: DeclId| {
            ast.decls.get(d).ty().and_then(|t| ast.types.proto(t)).map(|p| ast.types.exc_spec(p))
        };
        let mut cur = d;
        // At most one pattern per enclosing template level.
        for _ in 0..8 {
            match spec_of(cur) {
                Some(
                    ExcSpec::DependentNoexcept(c)
                    | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(c), .. },
                ) => {
                    self.byte(3);
                    return self.profile(c);
                }
                Some(ExcSpec::Deferred(n)) if n != cur => cur = n,
                // Waiting on itself: the declaration it is instantiated
                // from (a member template's function: its template's).
                Some(ExcSpec::Deferred(_)) => {
                    let from = ast.instantiated_from(ast.first(cur)).or_else(|| {
                        let t = ast.instantiated_from(ast.first(ast.described_template(cur)?))?;
                        ast.template_info(t)?.pattern
                    });
                    match from {
                        Some(p) => cur = p,
                        None => break,
                    }
                }
                Some(ExcSpec::Dynamic(_)) | None => break,
                Some(e) => return self.byte(u8::from(e.is_nothrow())),
            }
        }
        self.partial(Area::Dependent, Site::Item(Item::DependentNoexcept));
    }

    /// A dependent requires-expression by structure (WP-1626): its
    /// parameters' types, then per requirement its kind and operands; its
    /// parameters are named by position wherever it appears.
    pub(super) fn dep_requires(&mut self, e: ExprId, k: K) {
        use kiln_ast::templates::Requirement;
        let ast = self.ast;
        let Some(r) = ast.requires_of(e) else {
            return self.partial(Area::Dependent, Site::Expr(k));
        };
        self.expr_tag(k);
        self.positional += 1;
        self.count(ast.lists.len(r.params));
        for p in ast.lists.iter::<DeclId>(r.params) {
            match ast.decls.get(p).ty() {
                Some(t) => self.ty(t),
                None => self.byte(T_NONE),
            }
        }
        let reqs = ast.templates.requirements(r);
        self.count(reqs.len());
        for &q in reqs {
            match q {
                Requirement::Simple(x) => {
                    self.byte(1);
                    self.opt_expr_now(Some(x));
                }
                Requirement::Type(t, _) => {
                    self.byte(2);
                    self.ty(t);
                }
                Requirement::Compound { expr, noexcept, constraint } => {
                    self.byte(3);
                    self.byte(u8::from(noexcept.is_some()));
                    self.opt_expr_now(Some(expr));
                    self.opt_expr_now(constraint);
                }
                Requirement::Nested(x) => {
                    self.byte(4);
                    self.opt_expr_now(Some(x));
                }
            }
        }
        self.positional -= 1;
    }

    /// The dependent expression kinds (spelling-independent: qualifiers
    /// canonical, names by spelling where no entity exists yet, the
    /// declarations a lookup found not hashed, as [temp.over.link]).
    #[allow(clippy::many_single_char_names, reason = "short local names, one per AST item")]
    pub(super) fn dep_expr(&mut self, e: ExprId, k: K) {
        let ast = self.ast;
        let x = &ast.exprs;
        let n = *x.get(e);
        let id = ExprId::try_new;
        let nns_of = |e: ExprId| ast.templates.expr_nns.get(e).copied();
        let args_of = |e: ExprId| ast.templates.expr_targs.get(e).copied();
        match k {
            K::UnresolvedLookupExpr => {
                self.expr_tag(k);
                self.byte(n.aux() & 1);
                // A variable template template parameter by position.
                if let Some(d) = ast.lists.iter::<DeclId>(ListId::new(n.b())).next()
                    && ast.decls.get(d).kind() == DeclKind::TemplateTemplateParm
                {
                    self.tparm(d);
                } else {
                    self.decl_name(ast.unresolved_name(e));
                }
                self.nns(nns_of(e));
                self.dep_args(args_of(e));
            }
            K::DependentScopeDeclRefExpr => {
                self.expr_tag(k);
                self.nns(NnsId::from_raw(n.a()));
                self.decl_name(ast.dependent_scope_ref_name(e));
                self.dep_args(args_of(e));
            }
            K::CXXDependentScopeMemberExpr => {
                self.expr_tag(k);
                let base = id(n.a());
                self.byte(u8::from(base.is_some() && n.aux() & 1 != 0));
                self.decl_name(ast.dependent_member_name(e));
                self.nns(nns_of(e));
                self.dep_args(args_of(e));
                self.stack.push(Work::OptExpr(base));
            }
            K::SizeOfPackExpr => {
                self.expr_tag(k);
                match DeclId::from_raw(n.a()) {
                    Some(d) if ast.template_parm(d).is_some() => self.tparm(d),
                    Some(d) if ast.decls.get(d).kind() == DeclKind::ParmVar => self.parm_pos(d),
                    _ => self.partial(Area::Dependent, Site::Expr(k)),
                }
            }
            K::PackExpansionExpr => {
                self.expr_tag(k);
                self.stack.push(Work::OptExpr(id(n.a())));
            }
            K::CXXFoldExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.stack.extend([Work::OptExpr(id(n.a())), Work::OptExpr(id(n.b()))]);
            }
            K::CXXUnresolvedConstructExpr => {
                self.expr_tag(k);
                self.byte(n.aux() & 1);
                self.ty(x.unresolved_construct_type(e));
                self.exprs(ListId::new(n.a()));
            }
            // A dependent parenthesized initializer list (WP-1626).
            K::ParenListExpr => {
                self.expr_tag(k);
                self.exprs(ListId::new(n.a()));
            }
            K::ConceptSpecializationExpr => {
                self.expr_tag(k);
                match DeclId::from_raw(n.a()) {
                    Some(c) => self.template_entity(c),
                    None => self.byte(T_NONE),
                }
                self.dep_args(ArgListId::from_raw(n.b()));
            }
            _ => self.partial(Area::Dependent, Site::Expr(k)),
        }
    }
}
