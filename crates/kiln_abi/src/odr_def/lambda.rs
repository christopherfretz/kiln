//! Closures, local classes and local enumerations in the definition hash
//! (K-3d; mortar O-10 "Lambdas", O-14 "Closure members", ruling 5).
//!
//! A lambda-expression hashes as its closure's INDEX in the definition
//! (the closure type is a local entity, numbered at its first appearance
//! like a variable) and the closure's digest. The digest is the hash of
//! the closure's own serialization: the resolved capture list in closure
//! member order (kind, pack and init flags, the captured entity as the
//! enclosing scope names it), the initializers of the capture members as
//! expressions of the enclosing scope, then, in the closure's own scope
//! (the call operator's locals; captured entities by capture position),
//! the call operator's declarator (as for any function, `constexpr`
//! never), `mutable`, a generic lambda's template parameters and
//! requires-clause, and the call operator's body (a coroutine lambda's
//! wrapped `CoroutineBodyStmt`, `kiln-odr-2`). The digest is memoized per closure: the
//! row of the call operator (and of each specialization of a generic
//! one) is a function of it.
//!
//! A local class or enumeration hashes where it is declared: its index,
//! (`kiln-odr-2`) a class's union bit, `#pragma pack` value and layout
//! attributes or an enumeration's underlying type, its bases and its
//! members in declaration order, member functions'
//! definitions inline (their parameters and locals share the enclosing
//! numbering). Its member functions get no rows of their own.

use kiln_ast::decl::{DeclFlags, DeclKind, EnumData, TagKind};
use kiln_ast::expr::ExprKind as K;
use kiln_ast::ids::LambdaId;
use kiln_ast::lambda::{CaptureKind, LambdaData};
use kiln_ast::types::{ArgView, TypeView};
use kiln_ast::{Ast, DeclId, ExprId, QualType};

use super::{
    Area, DefDigest, DefHasher, Site, T_CALL_OP_ROW, T_CLOSURE, T_LOCAL_CLASS, T_LOCAL_MEMBER,
    T_LOCAL_TYPE, T_NONE, T_TU_PLACEHOLDER, Work, decl, fnv1a_128_bytes,
};

impl<'a> DefHasher<'a> {
    /// A lambda-expression: its closure's index (none for a closure of
    /// another context: a default member initializer's, a default
    /// argument's), then the closure's digest.
    pub(super) fn lambda_expr(&mut self, e: ExprId) {
        let ast = self.ast;
        let data = ast.exprs.lambda(e);
        self.expr_tag(K::LambdaExpr);
        match self.local(ast.first(data.closure)) {
            Some(i) => {
                self.byte(1);
                self.u32(i);
            }
            None => self.byte(0),
        }
        let d = self.closure_digest(data);
        self.byte(T_CLOSURE);
        self.out.extend_from_slice(&d.hash.to_le_bytes());
    }

    /// The digest of the closure of `data` (memoized; its `partial`
    /// joins the current definition's).
    fn closure_digest(&mut self, data: &'a LambdaData) -> DefDigest {
        let ast = self.ast;
        let key = ast.first(data.closure);
        if let Some(&d) = self.closures.get(&key) {
            self.partial |= d.partial;
            self.uses_internal |= d.uses_internal;
            self.refs.named_system |= d.names_system;
            return d;
        }
        // A closure naming its own type (an explicit object parameter)
        // meets this entry while its digest is computed.
        self.closures.insert(
            key,
            DefDigest { hash: 0, partial: false, uses_internal: false, names_system: false },
        );
        let start = self.out.len();
        let outer_partial = std::mem::replace(&mut self.partial, false);
        let outer_internal = std::mem::replace(&mut self.uses_internal, false);
        let outer_system = std::mem::replace(&mut self.refs.named_system, false);
        let base = self.stack.len();
        let floor = std::mem::replace(&mut self.floor, base);
        self.byte(T_CLOSURE);
        // The resolved captures, in the enclosing scope. The
        // capture-default is not hashed: `[=]` and `[x]` capturing only
        // `x` are equal.
        self.count(data.captures.len());
        for c in &data.captures {
            self.byte(c.kind as u8);
            self.byte(u8::from(c.init) | u8::from(c.ellipsis.is_valid()) << 1);
            match (c.kind, c.var) {
                (CaptureKind::ByCopy | CaptureKind::ByRef, Some(v)) if !c.init => self.decl_ref(v),
                _ => self.byte(T_NONE),
            }
        }
        match data.inits {
            Some(l) => {
                self.exprs(l);
                self.run_pushed(base);
            }
            None => self.byte(T_NONE),
        }
        match data.call_op {
            Some(op) => {
                self.enter_scope(op);
                for (i, c) in data.captures.iter().enumerate() {
                    if let Some(v) = c.var {
                        self.captures.push((v, u32::try_from(i).unwrap_or(u32::MAX)));
                    }
                }
                decl::declarator_as(self, op, true);
                self.byte(u8::from(data.mutable));
                self.generic_params(op);
                // The call operator's body: for a coroutine lambda, the
                // `CoroutineBodyStmt` sema wrapped the written body in
                // (`LambdaData::body` keeps the compound statement;
                // `kiln-odr-2`, WP-2361).
                let body = ast.decls.function(op).and_then(|f| f.body).or(data.body);
                self.stack.push(Work::OptStmt(body));
                self.run();
                self.leave_scope();
            }
            None => self.partial(Area::Lambda, Site::Expr(K::LambdaExpr)),
        }
        let d = DefDigest {
            hash: fnv1a_128_bytes(&self.out[start..]),
            partial: self.partial,
            uses_internal: self.uses_internal,
            names_system: self.refs.named_system,
        };
        self.out.truncate(start);
        self.partial = outer_partial | d.partial;
        self.uses_internal = outer_internal | d.uses_internal;
        self.refs.named_system = outer_system | d.names_system;
        self.floor = floor;
        self.closures.insert(key, d);
        d
    }

    /// A generic lambda's template parameters (kind, pack, a non-type
    /// parameter's type, the type-constraint) and requires-clause.
    fn generic_params(&mut self, op: DeclId) {
        let ast = self.ast;
        let Some(info) = ast.described_template(op).and_then(|t| ast.template_info(t)) else {
            return self.byte(T_NONE);
        };
        let pl = ast.templates.param_list(info.params);
        self.count(ast.lists.len(pl.params));
        for p in ast.lists.iter::<DeclId>(pl.params) {
            let n = ast.decls.get(p);
            let parm = ast.template_parm(p);
            self.byte(n.kind() as u8);
            self.byte(u8::from(parm.is_some_and(|x| x.pack)));
            if n.kind() == DeclKind::NonTypeTemplateParm {
                match n.ty() {
                    Some(t) => self.ty(t),
                    None => self.byte(T_NONE),
                }
            }
            self.stack.push(Work::OptExpr(parm.and_then(|x| x.constraint)));
            self.run();
        }
        self.stack.push(Work::OptExpr(pl.requires));
        self.run();
    }

    /// Reverses the items pushed since `base` (so they run in push order)
    /// and runs them.
    fn run_pushed(&mut self, base: usize) {
        self.stack[base..].reverse();
        self.run_nested(base);
    }

    /// Enters the scope of function `def` (a call operator): fresh locals
    /// and captures, the enclosing ones saved.
    fn enter_scope(&mut self, def: DeclId) {
        let mut s = self.spare.pop().unwrap_or_default();
        s.def = Some(self.def);
        std::mem::swap(&mut s.locals, &mut self.locals);
        std::mem::swap(&mut s.captures, &mut self.captures);
        std::mem::swap(&mut s.inline_defs, &mut self.inline_defs);
        self.scopes.push(s);
        self.def = self.ast.first(def);
    }

    /// Leaves the scope [`Self::enter_scope`] entered.
    pub(super) fn leave_scope(&mut self) {
        let Some(mut s) = self.scopes.pop() else { return };
        std::mem::swap(&mut s.locals, &mut self.locals);
        std::mem::swap(&mut s.captures, &mut self.captures);
        std::mem::swap(&mut s.inline_defs, &mut self.inline_defs);
        if let Some(d) = s.def.take() {
            self.def = d;
        }
        s.locals.clear();
        s.captures.clear();
        s.inline_defs.clear();
        self.spare.push(s);
    }

    /// The row of closure member `def` (the call operator of lambda `l`
    /// or a specialization of a generic one, O-14): the closure's digest
    /// inside its enclosing definition, then (for a specialization) its
    /// function type and template arguments.
    pub(super) fn call_op_row(&mut self, def: DeclId, l: LambdaId) -> DefDigest {
        let ast = self.ast;
        let data = ast.exprs.lambdas.get(l);
        let key = ast.first(data.closure);
        if !self.closures.contains_key(&key)
            && let Some(root) = closure_root(ast, data.closure)
        {
            // The walk of the enclosing definition fills the memo with
            // the digest under the enclosing numbering.
            self.walk_function(root);
        }
        let d = if let Some(&d) = self.closures.get(&key) {
            d
        } else {
            // Not reached from a function (a namespace-scope variable's,
            // a default member initializer's, a default argument's
            // closure: none can capture a local): its own.
            self.start(def);
            self.closure_digest(data)
        };
        self.start(def);
        self.byte(T_CALL_OP_ROW);
        self.out.extend_from_slice(&d.hash.to_le_bytes());
        self.partial = d.partial;
        self.uses_internal = d.uses_internal;
        self.refs.named_system = d.names_system;
        let spec = data.call_op.is_some_and(|op| ast.first(op) != ast.first(def));
        if spec {
            self.byte(1);
            if let Some(t) = ast.decls.get(def).ty() {
                self.ty(t);
            }
            if let Some((_, s)) = ast.spec_info(def) {
                self.targs(ast.types.canonical_args(s.args));
            }
        } else {
            self.byte(0);
        }
        self.finish()
    }

    /// Template arguments: a type by its encoding, an integral value by
    /// its value, anything else as the placeholder.
    fn targs(&mut self, list: kiln_ast::ids::ArgListId) {
        let ast = self.ast;
        let args = ast.types.args(list);
        self.count(args.len());
        for a in args {
            match a.view() {
                ArgView::Type(q) => self.ty(q),
                ArgView::Value { ty, value } => {
                    self.ty(ty);
                    match ast.consts.scalar(value) {
                        Some(kiln_ast::consts::Scalar::Int { bits, .. }) => {
                            self.count(bits.words().len());
                            for &w in bits.words() {
                                self.u64(w);
                            }
                        }
                        _ => self.byte(T_TU_PLACEHOLDER),
                    }
                }
                ArgView::Pack(p) => self.targs(p),
                _ => self.byte(T_TU_PLACEHOLDER),
            }
        }
    }

    /// The index of local entity `d` (`depth` 0: the current scope, with
    /// [`Self::local`]'s numbering; else an enclosing closure's scope,
    /// looked up only).
    fn scoped_index(&mut self, d: DeclId) -> Option<(u8, u32)> {
        if let Some(i) = self.local(d) {
            return Some((0, i));
        }
        self.outer_index(d)
    }

    /// The index of `d` in an enclosing scope.
    pub(super) fn outer_index(&self, d: DeclId) -> Option<(u8, u32)> {
        self.scopes.iter().rev().enumerate().find_map(|(depth, s)| {
            s.locals.get(&d).map(|&i| (u8::try_from(depth + 1).unwrap_or(u8::MAX), i))
        })
    }

    pub(super) fn write_index(&mut self, (depth, i): (u8, u32)) {
        self.byte(depth);
        self.u32(i);
    }

    /// A type the memo could not name (a component numbered per unit): a
    /// local class, enumeration or closure of the definition, possibly
    /// under pointers and references, by index; anything else the
    /// placeholder.
    pub(super) fn local_type(&mut self, q: QualType) {
        let ast = self.ast;
        let mut shape = [0u8; 8];
        let mut n = 0;
        let mut t = ast.types.canonical(q);
        loop {
            let quals = t.quals().bits();
            match ast.types.canonical_view(t) {
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p)
                    if n + 2 <= shape.len() =>
                {
                    shape[n] = ast.types.kind(t.ty()) as u8;
                    shape[n + 1] = quals;
                    n += 2;
                    t = p;
                }
                TypeView::Record(d) | TypeView::Enum(d) => {
                    let Some(ix) = self.scoped_index(ast.first(d)) else { break };
                    self.byte(T_LOCAL_TYPE);
                    self.byte(n as u8);
                    self.out.extend_from_slice(&shape[..n]);
                    self.byte(quals);
                    self.write_index(ix);
                    // A closure named only by its type (`decltype([]{})`)
                    // is hashed where it is named: its digest (memoized).
                    if let Some(l) = ast.closure_lambda(d) {
                        let dg = self.closure_digest(ast.exprs.lambdas.get(l));
                        self.out.extend_from_slice(&dg.hash.to_le_bytes());
                    }
                    return;
                }
                _ => break,
            }
        }
        match record_of(ast, t) {
            Some(rd) => self.tu_local_type(Some(rd)),
            // A type the walk does not take apart (a closure under a
            // function type): TU-local only when the mangler would not make
            // it visible; a closure of an inline function is the same
            // entity in every unit (WP-1638).
            None if !crate::mangle::type_externally_visible(ast, t) => self.tu_local_type(None),
            None => {}
        }
        self.byte(T_TU_PLACEHOLDER);
    }

    /// Class or enumeration `c` the memo could not name, as a type item
    /// (the owner of an enumerator or a member).
    pub(super) fn local_tag(&mut self, c: DeclId) {
        if let Some(ix) = self.scoped_index(self.ast.first(c)) {
            self.byte(T_LOCAL_TYPE);
            self.byte(0);
            self.byte(0);
            self.write_index(ix);
        } else {
            self.tu_local_type(Some(c));
            self.byte(T_TU_PLACEHOLDER);
        }
    }

    /// A function or variable the memo could not name: a member of a
    /// local class or closure as (class, name, kind, type); anything else
    /// the placeholder.
    pub(super) fn local_member(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        let Some(c) = n
            .ctx()
            .filter(|&c| matches!(ast.decls.get(c).kind(), DeclKind::CXXRecord | DeclKind::Enum))
        else {
            self.tu_local_entity(d);
            return self.byte(T_TU_PLACEHOLDER);
        };
        let Some(ix) = self.scoped_index(ast.first(c)) else {
            self.tu_local_entity(d);
            return self.byte(T_TU_PLACEHOLDER);
        };
        self.byte(T_LOCAL_MEMBER);
        self.write_index(ix);
        if let Some(op) = n.name().as_operator() {
            self.byte(1);
            self.byte(op as u8);
        } else {
            self.byte(0);
            self.name(d);
        }
        self.byte(n.kind() as u8);
        match n.ty() {
            Some(t) => self.ty(t),
            None => self.byte(T_NONE),
        }
    }

    /// A local class or enumeration's definition (from a declaration
    /// statement): its index, kind, then for a class its tag (`union` or
    /// not), `#pragma pack` value and layout attributes and its bases, for
    /// an enumeration its underlying type, then its members in
    /// declaration order, each with whatever its work pushes run before
    /// the next.
    pub(super) fn local_class(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        self.byte(T_LOCAL_CLASS);
        self.declare(ast.first(d));
        self.byte(n.kind() as u8);
        let base = self.stack.len();
        let floor = std::mem::replace(&mut self.floor, base);
        // `kiln-odr-2` (WP-2361): what a local class's layout depends on
        // besides its members (it has no kind 3 row: its type is TU-local),
        // and a local enumeration's underlying type.
        match n.kind() {
            DeclKind::CXXRecord => {
                let tag = ast.decls.record(d).map_or(TagKind::Struct, |r| r.tag);
                // `class` and `struct` declare the same kind of class.
                self.byte(u8::from(tag == TagKind::Union));
                let pack = ast.decls.cold.record_pack.get(d).copied().unwrap_or(0);
                self.u32(u32::from(pack));
                self.layout_attrs(d);
            }
            DeclKind::Enum => match ast.decls.enumeration(d).and_then(|e| e.int_type) {
                Some(t) => {
                    let fixed =
                        ast.decls.enumeration(d).is_some_and(|e| e.flags & EnumData::FIXED != 0);
                    self.byte(u8::from(fixed));
                    self.ty(t);
                }
                None => self.byte(T_NONE),
            },
            _ => {}
        }
        if n.kind() == DeclKind::CXXRecord {
            let bases = ast.decls.bases(d, &ast.lists);
            let mark = self.out.len();
            self.u32(0);
            let mut count = 0u32;
            for b in bases {
                count += 1;
                self.ty(b.ty);
                self.byte(u8::from(b.is_virtual) | (b.access as u8) << 1);
            }
            self.out[mark..mark + 4].copy_from_slice(&count.to_le_bytes());
        }
        for m in ast.members(d) {
            self.local_class_member(m);
            self.run();
        }
        self.byte(T_NONE);
        self.floor = floor;
    }

    fn local_class_member(&mut self, m: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(m);
        let k = n.kind();
        // Implicit members (the injected class name, implicitly declared
        // special members) are what the class's other items imply.
        if n.has(DeclFlags::IMPLICIT) {
            return;
        }
        match k {
            DeclKind::Field => {
                self.byte(k as u8);
                self.name(m);
                match n.ty() {
                    Some(t) => self.ty(t),
                    None => self.byte(T_NONE),
                }
                let f = ast.decls.field(m);
                self.u32(f.map_or(0, |f| f.flags));
                self.layout_attrs(m);
                self.stack.push(Work::OptExpr(f.and_then(|f| f.bit_width)));
                self.stack.push(Work::OptExpr(f.and_then(|f| f.init)));
                let base = self.stack.len() - 2;
                self.stack[base..].reverse();
            }
            DeclKind::EnumConstant => {
                self.byte(k as u8);
                self.name(m);
                let v = ast.decls.enumerator(m).and_then(|x| x.value);
                match v.and_then(|v| ast.consts.scalar(v)) {
                    Some(kiln_ast::consts::Scalar::Int { bits, .. }) => {
                        self.count(bits.words().len());
                        for &w in bits.words() {
                            self.u64(w);
                        }
                    }
                    _ => self.partial(Area::Misc, Site::Decl(k)),
                }
            }
            DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => {
                self.byte(k as u8);
                match n.name().as_operator() {
                    Some(op) => self.byte(op as u8),
                    None => self.name(m),
                }
                let first = ast.first(m);
                self.inline_defs.push(first);
                let def = ast.definition(first).unwrap_or(m);
                if decl::declarator(self, def) {
                    match k {
                        DeclKind::CXXConstructor => self.ctor_inits(def),
                        DeclKind::CXXDestructor => self.dealloc(first),
                        _ => {}
                    }
                    let body = ast.decls.function(def).and_then(|f| f.body);
                    self.stack.push(Work::OptStmt(body));
                }
            }
            DeclKind::CXXRecord | DeclKind::Enum => self.local_class(m),
            _ if !decl::decl_hashed(ast, m) => {}
            DeclKind::AccessSpec => {}
            _ => self.partial(Area::Lambda, Site::Decl(k)),
        }
    }
}

/// The definition whose walk reaches the closure type `rec`: the
/// outermost function enclosing it through call operators and local
/// classes, or the specialization of a generic call operator whose body
/// declares it; `None` outside any function.
fn closure_root(ast: &Ast, rec: DeclId) -> Option<DeclId> {
    let mut cand = None;
    let mut c = ast.decls.get(rec).ctx();
    for _ in 0..64 {
        let Some(x) = c else { break };
        let n = ast.decls.get(x);
        match n.kind() {
            DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => {
                if let Some(l) = ast.call_op_lambda(x) {
                    let lam = ast.exprs.lambdas.get(l);
                    if lam.call_op.is_some_and(|op| ast.first(op) != ast.first(x)) {
                        // A generic call operator's specialization.
                        return Some(x);
                    }
                    c = ast.decls.get(lam.closure).ctx();
                    continue;
                }
                cand = Some(x);
                c = n.ctx();
            }
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => c = n.ctx(),
            _ => break,
        }
    }
    cand
}

/// X15 (O-14): whether the declarator of function definition `def` holds
/// a lambda: a closure type of the declarator in its function type or
/// template arguments (`decltype([]{})` in a parameter or the return
/// type, a default template argument), or a lambda-expression in its
/// trailing requires-clause or a default argument, on the definition or
/// on the template it was instantiated from. Decided on the AST only.
pub fn lambda_in_declarator(ast: &Ast, def: DeclId) -> bool {
    let mut related = [def; 3];
    if let Some(p) = ast.instantiated_from(ast.first(def)) {
        related[1] = p;
    }
    if let Some((_, s)) = ast.spec_info(def) {
        related[2] = s.template;
        if let Some(p) = ast.template_info(s.template).and_then(|i| i.pattern) {
            related[1] = p;
        }
    }
    let related = related.map(|d| ast.first(d));
    let mut exprs = Vec::new();
    for &d in &related {
        for r in ast.redecls_of(d) {
            if let Some(f) = ast.decls.function(r) {
                for p in ast.lists.iter::<DeclId>(f.params) {
                    if let Some(e) = ast.decls.param(p).and_then(|x| x.default_arg) {
                        exprs.push(e);
                    }
                }
            }
            if let Some(&e) = ast.templates.trailing_requires.get(r) {
                exprs.push(e);
            }
            if let Some(t) = ast.decls.get(r).ty()
                && closure_in_type(ast, t, &related, 0)
            {
                return true;
            }
        }
    }
    if let Some((_, s)) = ast.spec_info(def)
        && closure_in_args(ast, ast.types.canonical_args(s.args), &related, 0)
    {
        return true;
    }
    while let Some(e) = exprs.pop() {
        if ast.exprs.get(e).kind() == K::LambdaExpr {
            return true;
        }
        ast.exprs.children_into(e, &ast.lists, &mut exprs);
    }
    false
}

/// Whether type `q` holds a closure type of the declarator of one of
/// `related` (see [`declarator_closure`]).
fn closure_in_type(ast: &Ast, q: QualType, related: &[DeclId; 3], depth: u32) -> bool {
    if depth > 16 {
        return false;
    }
    let t = ast.types.canonical(q);
    let deeper = |x: QualType| closure_in_type(ast, x, related, depth + 1);
    match ast.types.canonical_view(t) {
        TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => deeper(p),
        TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
            ast.types.element(t).is_some_and(deeper)
        }
        TypeView::MemberPointer { pointee, class } => {
            deeper(pointee) || deeper(QualType::unqual(class))
        }
        TypeView::Function(p) => deeper(p.ret()) || ast.types.params(&p).iter().any(|&x| deeper(x)),
        TypeView::Record(d) => {
            if ast.closure_lambda(d).is_some() {
                return declarator_closure(ast, d, related);
            }
            ast.spec_info(d).is_some_and(|(_, s)| {
                closure_in_args(ast, ast.types.canonical_args(s.args), related, depth + 1)
            })
        }
        _ => false,
    }
}

fn closure_in_args(
    ast: &Ast,
    list: kiln_ast::ids::ArgListId,
    related: &[DeclId; 3],
    depth: u32,
) -> bool {
    ast.types.args(list).iter().any(|a| match a.view() {
        ArgView::Type(q) => closure_in_type(ast, q, related, depth),
        ArgView::Pack(p) => closure_in_args(ast, p, related, depth + 1),
        _ => false,
    })
}

/// Whether closure type `rec` was declared by a lambda in a declarator
/// of one of `related`: its mangling context is one of them, or it has
/// none and no function or variable encloses it (a lambda of a
/// namespace-scope or class-scope signature). A closure of another
/// function's body or of a variable's initializer is an ordinary type
/// argument.
#[allow(clippy::many_single_char_names, reason = "short-lived walk variables")]
fn declarator_closure(ast: &Ast, rec: DeclId, related: &[DeclId; 3]) -> bool {
    let Some(l) = ast.closure_lambda(rec) else { return false };
    let m = ast.exprs.lambdas.get(l).mangling;
    if let Some(c) = m.context {
        return related.contains(&ast.first(c));
    }
    let mut c = ast.decls.get(rec).ctx();
    for _ in 0..64 {
        let Some(x) = c else { return true };
        let n = ast.decls.get(x);
        match n.kind() {
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => c = n.ctx(),
            DeclKind::Namespace
            | DeclKind::TranslationUnit
            | DeclKind::LinkageSpec
            | DeclKind::Export => return true,
            _ => return related.contains(&ast.first(x)),
        }
    }
    false
}

/// The class or enumeration canonical type `t` names, if it names one.
fn record_of(ast: &Ast, t: QualType) -> Option<DeclId> {
    match ast.types.canonical_view(t) {
        TypeView::Record(d) | TypeView::Enum(d) => Some(d),
        _ => None,
    }
}
