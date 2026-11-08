//! Argument-dependent lookup ([basic.lookup.argdep];
//! spec/sema-overload.md §2.2).

use kiln_ast::decl::DeclKind;
use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::DeclName;
use kiln_ast::types::{ArgView, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;

/// The associated namespaces and classes of a call's arguments
/// ([basic.lookup.argdep]/3), each a primary context / first declaration,
/// in discovery order.
#[derive(Default, Debug)]
pub(crate) struct AssocSet {
    /// Associated namespaces (primary contexts).
    pub namespaces: Vec<DeclId>,
    /// Associated classes (first declarations).
    pub classes: Vec<DeclId>,
    /// Associated enumerations (first declarations; [basic.lookup.argdep]/3.3).
    pub enums: Vec<DeclId>,
}

/// Bound on the type recursion (template arguments of template
/// arguments …); deeper parts add nothing.
const MAX_DEPTH: u32 = 64;

pub(crate) fn is_class_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

impl Sema<'_, '_> {
    /// The innermost namespace (or the translation unit) enclosing `d`,
    /// as a primary context.
    pub(crate) fn innermost_namespace(&self, d: DeclId) -> Option<DeclId> {
        let mut cur = self.ast.decls.get(d).ctx();
        while let Some(c) = cur {
            if matches!(self.kind_of(c), DeclKind::Namespace | DeclKind::TranslationUnit) {
                return Some(self.ast.primary_context(c));
            }
            cur = self.ast.decls.get(c).ctx();
        }
        None
    }

    fn assoc_namespace(&self, ns: DeclId, out: &mut AssocSet) {
        if out.namespaces.contains(&ns) {
            return;
        }
        out.namespaces.push(ns);
        // [basic.lookup.argdep]/3: an inline namespace adds its enclosing
        // namespace, and a namespace its inline namespaces.
        if self.kind_of(ns) == DeclKind::Namespace
            && self
                .ast
                .decls
                .namespace(self.ast.first(ns))
                .is_some_and(kiln_ast::decl::NamespaceData::is_inline)
            && let Some(p) = self.ast.decls.get(ns).ctx()
            && matches!(self.kind_of(p), DeclKind::Namespace | DeclKind::TranslationUnit)
        {
            self.assoc_namespace(self.ast.primary_context(p), out);
        }
        let children = self.ast.lookup.inline_children(ns).to_vec();
        for c in children {
            self.assoc_namespace(c, out);
        }
    }

    fn assoc_entity_ns(&self, d: DeclId, out: &mut AssocSet) {
        if let Some(ns) = self.innermost_namespace(d) {
            self.assoc_namespace(ns, out);
        }
    }

    /// Adds class `c` (and nothing else) as an associated class.
    fn assoc_class_only(&self, c: DeclId, out: &mut AssocSet) -> bool {
        let f = self.ast.first(c);
        if out.classes.contains(&f) {
            return false;
        }
        out.classes.push(f);
        self.assoc_entity_ns(c, out);
        true
    }

    /// [basic.lookup.argdep]/3.2: the class, the class it is a member of,
    /// its bases; a specialization's template arguments (its own only:
    /// not a base's or the enclosing class's, as clang
    /// `addAssociatedClassesAndNamespaces`).
    fn assoc_class(&self, c: DeclId, out: &mut AssocSet, depth: u32) {
        if !self.assoc_class_only(c, out) {
            return;
        }
        if let Some(p) = self.ast.decls.get(c).ctx()
            && is_class_kind(self.kind_of(p))
        {
            self.assoc_class_only(p, out);
        }
        if let Some((_, s)) = self.ast.spec_info(c) {
            let args = s.args;
            self.assoc_args(args, out, depth + 1);
        }
        let def = self.ast.definition(c).unwrap_or(c);
        let mut stack = vec![def];
        while let Some(x) = stack.pop() {
            let bases: Vec<QualType> =
                self.ast.decls.bases(x, &self.ast.lists).map(|b| b.ty).collect();
            for bt in bases {
                if let Some(b) = self.ast.types.as_record(bt)
                    && self.assoc_class_only(b, out)
                {
                    stack.push(self.ast.definition(b).unwrap_or(b));
                }
            }
        }
    }

    fn assoc_args(&self, l: kiln_ast::ArgListId, out: &mut AssocSet, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let args = self.ast.types.args(l).to_vec();
        for a in args {
            match a.view() {
                ArgView::Type(t) => self.assoc_type(t, out, depth + 1),
                ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                    // /3.2: a template template argument's template: its
                    // namespace, and its class if a member.
                    if let Some(d) = self.ast.types.template_decl(n) {
                        if let Some(p) = self.ast.decls.get(d).ctx()
                            && is_class_kind(self.kind_of(p))
                        {
                            self.assoc_class_only(p, out);
                        }
                        self.assoc_entity_ns(d, out);
                    }
                }
                ArgView::Pack(p) => self.assoc_args(p, out, depth + 1),
                _ => {}
            }
        }
    }

    /// The associated entities of type `t` ([basic.lookup.argdep]/3).
    pub(crate) fn assoc_type(&self, t: QualType, out: &mut AssocSet, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        match self.ast.types.canonical_view(t) {
            TypeView::Pointer(p)
            | TypeView::LValueRef(p)
            | TypeView::RValueRef(p)
            | TypeView::IncompleteArray(p)
            | TypeView::Atomic(p) => self.assoc_type(p, out, depth + 1),
            TypeView::ConstantArray(a) => self.assoc_type(a.elem, out, depth + 1),
            TypeView::Function(f) => {
                let ps: Vec<QualType> = self.ast.types.params(&f).to_vec();
                for p in ps {
                    self.assoc_type(p, out, depth + 1);
                }
                self.assoc_type(f.ret(), out, depth + 1);
            }
            TypeView::MemberPointer { pointee, class } => {
                self.assoc_type(QualType::from(class), out, depth + 1);
                self.assoc_type(pointee, out, depth + 1);
            }
            TypeView::Record(d) => self.assoc_class(d, out, depth),
            TypeView::Enum(d) => {
                // /3.3: the innermost enclosing namespace; the class of
                // which it is a member.
                if let Some(p) = self.ast.decls.get(d).ctx()
                    && is_class_kind(self.kind_of(p))
                {
                    self.assoc_class_only(p, out);
                }
                let f = self.ast.first(d);
                if !out.enums.contains(&f) {
                    out.enums.push(f);
                }
                self.assoc_entity_ns(d, out);
            }
            // [basic.lookup.argdep]/3.1 (P2996R13): the associated set
            // of std::meta::info is the enumeration std::meta::operators,
            // so std::meta is its associated namespace and an
            // unqualified `type_of(r)` finds std::meta::type_of.
            TypeView::Builtin(_) if self.is_meta_info(t) => self.assoc_meta_info(out),
            _ => {}
        }
    }

    /// [`Self::assoc_type`] of `std::meta::info`: `std::meta::operators`
    /// and namespace `std::meta`, once `<meta>` declared them.
    fn assoc_meta_info(&self, out: &mut AssocSet) {
        let ns = if let Some(ns) = self.std_meta_ns.get() {
            ns
        } else {
            let std = self.intern("std");
            let found = self.lookup_qualified(self.ast.tu, std, LookupKind::Nested);
            let Some(std) =
                found.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)
            else {
                return;
            };
            let meta = self.intern("meta");
            let found = self.lookup_qualified(std, meta, LookupKind::Nested);
            let Some(ns) =
                found.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)
            else {
                return;
            };
            let ns = self.ast.primary_context(ns);
            self.std_meta_ns.set(Some(ns));
            ns
        };
        let operators = self.intern("operators");
        let found = self.lookup_qualified(ns, operators, LookupKind::Ordinary);
        if let Some(e) = found.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Enum) {
            let f = self.ast.first(e);
            if !out.enums.contains(&f) {
                out.enums.push(f);
            }
        }
        self.assoc_namespace(ns, out);
    }

    /// The associated entities of argument `e`; an overload-set argument
    /// adds those of every function it names (/3, last bullet).
    pub(crate) fn assoc_arg(&self, e: ExprId, out: &mut AssocSet) {
        let x = self.ignore_parens(e);
        let n = self.ast.exprs.get(x);
        if n.kind() == kiln_ast::expr::ExprKind::UnresolvedLookupExpr {
            for d in self.ast.unresolved_decls(x) {
                if let Some(t) = self.ast.decls.get(d).ty() {
                    self.assoc_type(t, out, 0);
                }
            }
            return;
        }
        self.assoc_type(n.ty(), out, 0);
    }

    /// Argument-dependent lookup of `name` (an identifier or an
    /// `operator@`, [over.match.oper]/3.2) for a call with `args`
    /// ([basic.lookup.argdep]/4): the functions and function templates
    /// declared in the associated namespaces (using-directives ignored),
    /// including hidden friends of the associated classes, that `out`
    /// does not already hold (compared by first declaration). Appends
    /// `(decl, found)` pairs to `out`.
    ///
    /// Each associated class that is an undefined class template
    /// specialization is instantiated first, from `loc` (clang
    /// `addAssociatedClassesAndNamespaces` calls `isCompleteType`): its
    /// hidden friends ([temp.inst]/3) and bases are then associated.
    pub(crate) fn adl_lookup_dn(
        &mut self,
        name: DeclName,
        args: &[ExprId],
        loc: Loc,
        out: &mut Vec<(DeclId, DeclId)>,
    ) {
        let mut assoc = AssocSet::default();
        for &a in args {
            self.assoc_arg(a, &mut assoc);
        }
        // Instantiating a class can associate more (its bases); bounded.
        for _ in 0..MAX_DEPTH {
            let pending: Vec<DeclId> = assoc
                .classes
                .iter()
                .copied()
                .filter(|&c| {
                    (self.ast.spec_info(c).is_some() || self.ast.instantiated_from(c).is_some())
                        && self.ast.definition(c).is_none()
                        && !self.ast.decls.get(c).has(kiln_ast::decl::DeclFlags::INVALID)
                })
                .collect();
            if pending.is_empty() || self.inst.fatal {
                break;
            }
            let mut grew = false;
            for c in pending {
                grew |= self.instantiate_class(c, loc, false) && self.ast.definition(c).is_some();
            }
            if !grew {
                break;
            }
            assoc = AssocSet::default();
            for &a in args {
                self.assoc_arg(a, &mut assoc);
            }
        }
        for &ns in &assoc.namespaces {
            let r = self.ast.lookup_local(ns, name);
            for (raw, flags) in r.iter_with_flags() {
                let d = self.ast.strip_shadow(raw);
                let k = self.kind_of(d);
                if !(k == DeclKind::Function || k == DeclKind::FunctionTemplate) {
                    continue;
                }
                if flags.contains(LookupFlags::ADL_ONLY) {
                    if !self.friend_of_any(d, &assoc.classes) {
                        continue;
                    }
                } else if !self.module_adl_visible(raw, &assoc) {
                    continue;
                }
                let f = self.ast.first(d);
                if out.iter().any(|&(o, _)| self.ast.first(o) == f) {
                    continue;
                }
                out.push((d, raw));
            }
        }
    }

    /// True if some declaration of `d` is lexically in one of `classes`
    /// (a friend declaration, [basic.lookup.argdep]/4.2). A template's
    /// declarations are its pattern's: the template decl that holds the
    /// lookup entry is only the latest one's.
    fn friend_of_any(&self, d: DeclId, classes: &[DeclId]) -> bool {
        let in_assoc = |x: DeclId| {
            self.ast.decls.lexical_ctx(x).is_some_and(|l| classes.contains(&self.ast.first(l)))
        };
        if in_assoc(d) {
            return true;
        }
        let mut cur = Some(self.pattern_of(d).unwrap_or(d));
        while let Some(x) = cur {
            if in_assoc(x) {
                return true;
            }
            cur = self.ast.prev(x);
        }
        false
    }
}
