//! The clang nullability qualifiers `_Nonnull`, `_Nullable`,
//! `_Null_unspecified` and `_Nullable_result` (a clang extension;
//! spec/sema.md §4.1.1). They are recorded as `Attributed` type sugar over
//! the pointer they qualify, as clang's `AttributedType`, and checked as
//! clang's `CheckNullabilityTypeSpecifier` and
//! `distributeNullabilityTypeAttr` do (SemaType.cpp).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::types::{TypeAttrKind, TypeView};
use kiln_ast::{AttrId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{Chunk, DeclSpec, Declarator, DeclaratorContext, NullSpec, ParamInfo};
use kiln_ast::DeclId;

/// True for the four nullability attribute kinds.
pub(crate) fn is_nullability(kind: TypeAttrKind) -> bool {
    matches!(
        kind,
        TypeAttrKind::Nonnull
            | TypeAttrKind::Nullable
            | TypeAttrKind::NullUnspecified
            | TypeAttrKind::NullableResult
    )
}

/// The keyword spelling of a nullability kind.
pub(crate) fn spelling(kind: TypeAttrKind) -> &'static str {
    match kind {
        TypeAttrKind::Nonnull => "_Nonnull",
        TypeAttrKind::Nullable => "_Nullable",
        TypeAttrKind::NullUnspecified => "_Null_unspecified",
        _ => "_Nullable_result",
    }
}

/// Where a decl-specifier nullability qualifier moves to
/// (`warn_nullability_declspec`'s `%select`).
#[derive(Copy, Clone)]
enum PointerKind {
    Pointer = 0,
    MemberPointer = 2,
    FunctionPointer = 3,
    MemberFunctionPointer = 4,
}

impl Sema<'_, '_> {
    /// clang's `Type::canHaveNullability`: a pointer or member pointer,
    /// or a dependent type that may become one.
    /// A class marked `class _Nullable X` (clang nullability on classes)
    /// also can: a non-template class when any redeclaration carries the
    /// mark; a class template specialization, dependent or not, only when
    /// a redeclaration of its primary template's pattern does (a mark on
    /// a partial or explicit specialization does not count, as in clang
    /// 19). The injected-class-name inside the template cannot.
    pub(crate) fn can_have_nullability(&self, ty: QualType) -> bool {
        let t = &self.ast.types;
        let c = t.canonical(ty);
        match t.view(c.ty()) {
            TypeView::Record(d) => {
                return match self.ast.spec_info(d) {
                    Some((_, s)) => self.class_template_nullable(s.template),
                    None => self.ast.redecls_of(d).take(4096).any(|x| self.marked_nullable(x)),
                };
            }
            TypeView::TemplateSpecialization { name, alias: false, .. } => {
                if let Some(td) = t.template_decl(name) {
                    // `X<T>` inside `X` is clang's `InjectedClassNameType`.
                    return self.current_instantiation(td, c).is_none()
                        && self.class_template_nullable(td);
                }
            }
            _ => {}
        }
        matches!(
            t.view(c.ty()),
            TypeView::Pointer(_)
                | TypeView::MemberPointer { .. }
                | TypeView::TemplateTypeParm(_)
                | TypeView::CanonTemplateTypeParm { .. }
                | TypeView::SubstTemplateTypeParmPack { .. }
                | TypeView::DependentName { .. }
                | TypeView::DependentTemplateSpecialization { .. }
                | TypeView::UnresolvedUsing(_)
                | TypeView::PackIndexing { .. }
                | TypeView::Decltype { .. }
                | TypeView::DependentDecltype(_)
                | TypeView::TypeOf { .. }
                | TypeView::UnaryTransform { .. }
                | TypeView::Auto { .. }
                | TypeView::Splice { .. }
        )
    }

    /// Some redeclaration of class template `td` has a `_Nullable`
    /// pattern.
    fn class_template_nullable(&self, td: DeclId) -> bool {
        self.ast.redecls_of(td).take(4096).any(|x| {
            self.ast
                .template_info(x)
                .and_then(|i| i.pattern)
                .is_some_and(|p| self.marked_nullable(p))
        })
    }

    /// Declaration `d` itself carries `class _Nullable`, or is one of the
    /// standard library classes clang marks implicitly.
    fn marked_nullable(&self, d: DeclId) -> bool {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::TYPE_NULLABLE)
            || self.std_nullable_class(d)
    }

    /// clang's `inferNullableClassAttribute`: a class (or class template
    /// pattern) of one of these names directly in `std` (inline
    /// namespaces skipped) is nullable without a written mark, so
    /// `_Nonnull std::unique_ptr<int>` is valid.
    fn std_nullable_class(&self, d: DeclId) -> bool {
        const NAMES: [&str; 7] = [
            "auto_ptr",
            "coroutine_handle",
            "exception_ptr",
            "function",
            "move_only_function",
            "shared_ptr",
            "unique_ptr",
        ];
        self.kind_of(d) == DeclKind::CXXRecord
            && self
                .ast
                .decls
                .get(d)
                .name()
                .as_ident()
                .is_some_and(|n| NAMES.iter().any(|&s| self.intern(s) == n))
            && self.is_in_std_namespace(d)
    }

    /// `class _Nullable` after a class-key: a keyword attribute of the
    /// class, checked with the others ([`Sema::check_decl_attrs`]).
    pub fn act_on_class_nullable(&mut self, loc: Loc) -> AttrId {
        let node = AttrNode {
            kind: AttrKind::TYPE_NULLABLE,
            syntax: AttrSyntax::Keyword,
            flags: 0,
            loc,
            a: 0,
            b: 0,
        };
        self.ast.attrs.push(node)
    }

    /// The nullability `ty` carries, looking through sugar (clang's
    /// `Type::getNullability`), and the sugar node just below the
    /// outermost written `Attributed` layers where it was found.
    fn sugared_nullability(&self, mut ty: QualType) -> Option<TypeAttrKind> {
        let t = &self.ast.types;
        for _ in 0..256 {
            if let TypeView::Attributed { kind, .. } = t.view(ty.ty())
                && is_nullability(kind)
            {
                return Some(kind);
            }
            let n = t.desugar_once(ty);
            if n == ty {
                return None;
            }
            ty = n;
        }
        None
    }

    /// The outermost typedef in `ty`'s sugar (clang's
    /// `getAs<TypedefType>`) and its underlying type.
    fn first_typedef(&self, mut ty: QualType) -> Option<(DeclId, QualType)> {
        let t = &self.ast.types;
        for _ in 0..256 {
            match t.view(ty.ty()) {
                TypeView::Typedef { decl, underlying } => return Some((decl, underlying)),
                TypeView::Attributed { .. } => return None,
                _ => {}
            }
            let n = t.desugar_once(ty);
            if n == ty {
                return None;
            }
            ty = n;
        }
        None
    }

    /// Applies the nullability qualifier `n` to `ty` (clang's
    /// `CheckNullabilityTypeSpecifier`): a duplicate warns, a conflict
    /// with one already on the type (written, or through a typedef) and
    /// a non-pointer type are errors that drop the qualifier.
    /// `allow_array` admits an array (a parameter's, which decays).
    pub(crate) fn apply_nullability(
        &mut self,
        ty: QualType,
        n: NullSpec,
        allow_array: bool,
    ) -> QualType {
        if self.is_error_type(ty) {
            return ty;
        }
        let sp = spelling(n.kind);
        let mut desugared = ty;
        while let TypeView::Attributed { kind, modified, .. } = self.ast.types.view(desugared.ty())
        {
            if is_nullability(kind) {
                if kind == n.kind {
                    let d = self.diag(DiagId::sema_nullability_duplicate, n.loc).arg(sp);
                    self.emit(d);
                    break;
                }
                let d = self
                    .diag(DiagId::sema_nullability_conflicting, n.loc)
                    .arg(sp)
                    .arg(spelling(kind));
                self.emit(d);
                return ty;
            }
            desugared = modified;
        }
        if let Some(existing) = self.sugared_nullability(desugared)
            && existing != n.kind
        {
            let mut d = self
                .diag(DiagId::sema_nullability_conflicting, n.loc)
                .arg(sp)
                .arg(spelling(existing));
            // The typedef whose type carries it.
            if let Some((decl, underlying)) = self.first_typedef(desugared)
                && let TypeView::Attributed { kind, .. } = self.ast.types.view(underlying.ty())
                && kind == existing
            {
                let at = self.ast.decls.get(decl).loc();
                d = d.with_note(
                    self.diag(DiagId::note_nullability_here, at).arg(spelling(existing)),
                );
            }
            self.emit(d);
            return ty;
        }
        let array =
            allow_array && self.ast.types.kind(self.ast.types.canonical(desugared).ty()).is_array();
        if !self.can_have_nullability(desugared) && !array {
            let d =
                self.diag(DiagId::sema_nullability_nonpointer, n.loc).arg(sp).arg(self.ty_arg(ty));
            self.emit(d);
            return ty;
        }
        let r = self.ast.types.attributed(n.kind, ty, ty);
        if self.is_dependent_type(ty) && n.loc.is_valid() {
            self.inst.nullability_locs.entry(r).or_default().push(n.loc);
        }
        r
    }

    /// The location of the nullability qualifier of the dependent type
    /// `t` written nearest `near` (a substituted declaration's), else
    /// `near` (see `InstState::nullability_locs`).
    pub(crate) fn nullability_loc(&self, t: QualType, near: Loc) -> Loc {
        self.inst
            .nullability_locs
            .get(&t)
            .and_then(|v| v.iter().copied().min_by_key(|l| l.to_raw().abs_diff(near.to_raw())))
            .unwrap_or(near)
    }

    /// The decl-specifier nullability qualifiers of `ds` applied to its
    /// type `ty`. One on a type that cannot carry it may belong to a
    /// declarator's pointer (clang's `distributeNullabilityTypeAttr`):
    /// it is kept as an unchecked marker layer for
    /// [`Sema::declarator_nullability`], which knows the declarator.
    /// One on an undeduced `auto` is dropped (spec/sema.md §4.1.1).
    pub(crate) fn declspec_nullability(&mut self, ds: &DeclSpec, mut ty: QualType) -> QualType {
        for &n in &ds.nullability {
            if self.is_error_type(ty) {
                return ty;
            }
            let t = &self.ast.types;
            if matches!(t.view(t.canonical(ty).ty()), TypeView::Auto { deduced: None, .. }) {
                continue;
            }
            if self.can_have_nullability(ty) || self.is_dependent_type(ty) {
                ty = self.apply_nullability(ty, n, false);
            } else {
                let m = self.ast.types.attributed(n.kind, ty, ty);
                self.declspec_nulls.retain(|&(t, _)| t != m);
                self.declspec_nulls.push((m, n));
                ty = m;
            }
        }
        ty
    }

    /// Strips the decl-specifier nullability markers
    /// ([`Sema::declspec_nullability`]) from `base` and resolves them
    /// against `d`: each moves to the declarator's outermost pointer
    /// (warning `-Wnullability-declspec`, an error by default) or is
    /// checked on `base`. Returns the base type and the qualifiers moved
    /// to chunks, by chunk index.
    pub(crate) fn declarator_nullability(
        &mut self,
        mut base: QualType,
        spec_loc: Loc,
        d: &Declarator,
    ) -> (QualType, Vec<(usize, NullSpec)>) {
        let mut marks = Vec::new();
        while let TypeView::Attributed { kind, modified, .. } = self.ast.types.view(base.ty())
            && is_nullability(kind)
            && !self.can_have_nullability(modified)
            && !self.is_dependent_type(modified)
        {
            let loc = self
                .declspec_nulls
                .iter()
                .rev()
                .find(|&&(m, _)| m == base)
                .map_or(spec_loc, |&(_, n)| n.loc);
            marks.push(NullSpec { kind, loc });
            base = modified;
        }
        let mut moved: Vec<(usize, NullSpec)> = Vec::new();
        for &n in marks.iter().rev() {
            if self.ast.types.kind(self.ast.types.canonical(base).ty()).is_array() {
                let allow = d.ctx == DeclaratorContext::Prototype
                    && !d.chunks.iter().any(|c| {
                        matches!(
                            c,
                            Chunk::Array { .. }
                                | Chunk::Pointer { .. }
                                | Chunk::MemberPointer { .. }
                                | Chunk::LRef { .. }
                                | Chunk::RRef { .. }
                        )
                    });
                base = self.apply_nullability(base, n, allow);
                continue;
            }
            let target = Self::nullability_target(d)
                .filter(|&(i, _)| !d.nullability.iter().chain(moved.iter()).any(|&(j, _)| j == i));
            match target {
                Some((i, pk)) => {
                    let d = self
                        .diag(DiagId::sema_nullability_declspec, n.loc)
                        .arg(spelling(n.kind))
                        .arg(self.ty_arg(base))
                        .arg(pk as u8);
                    self.emit(d);
                    moved.push((i, n));
                }
                None => base = self.apply_nullability(base, n, false),
            }
        }
        (base, moved)
    }

    /// The chunk a decl-specifier nullability qualifier moves to (clang's
    /// `distributeNullabilityTypeAttr`): the outermost pointer or member
    /// pointer, through arrays, past a function's return type to a
    /// function pointer; not through a reference.
    fn nullability_target(d: &Declarator) -> Option<(usize, PointerKind)> {
        let mut i = d.chunks.len();
        while i != 0 {
            match &d.chunks[i - 1] {
                Chunk::Pointer { .. } => return Some((i - 1, PointerKind::Pointer)),
                Chunk::MemberPointer { .. } => return Some((i - 1, PointerKind::MemberPointer)),
                Chunk::Array { .. } => i -= 1,
                Chunk::LRef { .. } | Chunk::RRef { .. } => return None,
                Chunk::Function { .. } => {
                    return Self::past_return_type(d, i).map(|(j, member)| {
                        let pk = if member {
                            PointerKind::MemberFunctionPointer
                        } else {
                            PointerKind::FunctionPointer
                        };
                        (j, pk)
                    });
                }
            }
        }
        None
    }

    /// clang's `maybeMovePastReturnType`: from the function chunk
    /// `i - 1` inwards, the (member) pointer to that function.
    fn past_return_type(d: &Declarator, mut i: usize) -> Option<(usize, bool)> {
        let mut result = None;
        while i != 0 {
            match &d.chunks[i - 1] {
                Chunk::Function { .. } => {
                    i -= 1;
                    let mut found = false;
                    while i != 0 {
                        match &d.chunks[i - 1] {
                            Chunk::Pointer { .. } => {
                                result = Some((i - 1, false));
                                found = true;
                                break;
                            }
                            Chunk::MemberPointer { .. } => {
                                result = Some((i - 1, true));
                                found = true;
                                break;
                            }
                            _ => i -= 1,
                        }
                    }
                    if !found {
                        return result;
                    }
                }
                _ => return result,
            }
            i -= 1;
        }
        result
    }

    /// A redeclaration's parameters against the latest declaration of
    /// `prev` (clang's `mergeParamDeclAttributes`): an outermost
    /// nullability that differs from the previous one is
    /// `-Wnullability` "conflicts with existing specifier", at the new
    /// parameter, noting the old. A parameter written without one
    /// inherits the previous one (clang copies it onto the type), so the
    /// nearest earlier declaration that wrote one decides. Only
    /// parameters are compared (a return type is not, in C++).
    pub(crate) fn check_param_nullability(&mut self, prev: DeclId, params: &[ParamInfo]) {
        let redecls: Vec<DeclId> = self.ast.redecls_of(prev).collect();
        let param_at = |s: &Self, f: DeclId, i: usize| {
            s.ast.decls.function(f).and_then(|x| s.ast.lists.at::<DeclId>(x.params, i))
        };
        for (i, p) in params.iter().enumerate() {
            let Some(new) = self.sugared_nullability(p.ty) else { continue };
            let Some(latest) = redecls.first().and_then(|&f| param_at(self, f, i)) else {
                continue;
            };
            let was = redecls.iter().find_map(|&f| {
                let t = self.ast.decls.get(param_at(self, f, i)?).ty()?;
                self.sugared_nullability(t)
            });
            if let Some(was) = was
                && new != was
            {
                let at = self.ast.decls.get(latest).loc();
                let d = self
                    .diag(DiagId::sema_nullability_mismatched_redecl, p.loc)
                    .arg(spelling(new))
                    .arg(spelling(was))
                    .with_note(self.diag(DiagId::note_previous_declaration, at));
                self.emit(d);
            }
        }
    }

    /// The nullability qualifiers written after (or moved to) chunk `i`
    /// of `d`, applied to its type `ty`.
    pub(crate) fn chunk_nullability(
        &mut self,
        mut ty: QualType,
        i: usize,
        d: &Declarator,
        moved: &[(usize, NullSpec)],
    ) -> QualType {
        // One in a parameter's outermost array brackets (`int a[_Nonnull]`)
        // applies to the array, which decays ([dcl.fct]/5).
        let param_array = i == 0
            && d.ctx == DeclaratorContext::Prototype
            && matches!(d.chunks.first(), Some(Chunk::Array { .. }));
        for &(j, n) in d.nullability.iter().chain(moved) {
            if j == i {
                ty = self.apply_nullability(ty, n, param_array);
            }
        }
        ty
    }
}
