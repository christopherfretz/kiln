//! Why a type trait is false (spec/sema-templates.md §9.5): the notes
//! clang 22 adds after a failed `static_assert` requirement or atomic
//! constraint that is a type trait (clang `DiagnoseTypeTraitDetails`).
//! The trait is `__trait(T...)`, `std::is_trait_v<T...>` or
//! `std::is_trait<T...>::value` (clang `ExtractTypeTraitFromExpression`).
//! Explained so far: `__is_trivially_copyable`
//! (`DiagnoseNonTriviallyCopyableReason`).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprKind, TypeTrait};
use kiln_ast::types::ArgView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::class::SpecialKind;
use crate::templ::concepts::SatRecord;

impl Sema<'_, '_> {
    /// The notes explaining why `e` (a false condition) is false, at
    /// `at`, when it is a type trait clang 22 explains; none below
    /// `-fclang-compat=22`.
    pub(crate) fn trait_explanation(&self, e: ExprId, at: Loc) -> Vec<SatRecord> {
        let mut out = Vec::new();
        if self.clang_compat < 22 {
            return out;
        }
        let Some((t, args)) = self.extract_type_trait(e) else { return out };
        match (t, args.as_slice()) {
            (TypeTrait::IsTriviallyCopyable, &[q]) => {
                self.explain_trivially_copyable(q, at, &mut out)
            }
            _ => {}
        }
        out
    }

    /// `d` (a failed `static_assert` requirement `e`) with the notes
    /// explaining its type trait, at `e`.
    pub(crate) fn with_trait_explanation(&self, d: kiln_diag::Diag, e: ExprId) -> kiln_diag::Diag {
        let recs = self.trait_explanation(e, self.begin_loc(e));
        let mut notes = Vec::new();
        self.render_records(&recs, &mut notes, 0);
        notes.into_iter().fold(d, kiln_diag::Diag::with_note)
    }

    /// `e`'s trait and type operands (clang
    /// `ExtractTypeTraitFromExpression`).
    fn extract_type_trait(&self, e: ExprId) -> Option<(TypeTrait, Vec<QualType>)> {
        let e = self.ignore_paren_imp_casts(e);
        match self.ast.exprs.get(e).kind() {
            ExprKind::TypeTraitExpr => {
                let args = self.ast.exprs.type_trait_args(e);
                Some((
                    self.ast.exprs.type_trait(e),
                    self.ast.lists.iter::<QualType>(args).collect(),
                ))
            }
            ExprKind::DeclRefExpr => {
                let d = self.ast.exprs.decl(e);
                // `std::is_trait_v<T...>`.
                if self.kind_of(d) == DeclKind::Var
                    && let Some((_, info)) = self.ast.spec_info(d)
                {
                    let name = self.decl_name(info.template);
                    let t = std_trait(name.strip_suffix("_v")?)?;
                    return self.is_in_std_namespace(d).then(|| (t, self.type_args(info.args)));
                }
                // `std::is_trait<T...>::value`.
                if self.decl_name(d) != "value" {
                    return None;
                }
                let class = self.ast.decls.get(d).ctx()?;
                let (_, info) = self.ast.spec_info(class)?;
                let t = std_trait(&self.decl_name(info.template))?;
                self.is_in_std_namespace(class).then(|| (t, self.type_args(info.args)))
            }
            _ => None,
        }
    }

    /// The type arguments of `args`.
    fn type_args(&self, args: kiln_ast::ids::ArgListId) -> Vec<QualType> {
        self.ast
            .types
            .args(args)
            .iter()
            .filter_map(|a| match a.view() {
                ArgView::Type(q) => Some(q),
                _ => None,
            })
            .collect()
    }

    /// clang `DiagnoseNonTriviallyCopyableReason`.
    fn explain_trivially_copyable(&self, q: QualType, at: Loc, out: &mut Vec<SatRecord>) {
        let note = |id: DiagId, loc: Loc, args: Vec<String>| SatRecord {
            id,
            loc,
            select: false,
            args,
            nested: None,
        };
        out.push(note(
            DiagId::note_unsatisfied_trait,
            at,
            vec![self.ty_arg(q), "trivially copyable".into()],
        ));
        if self.ast.types.is_reference(q) {
            out.push(note(DiagId::note_unsatisfied_trait_reference, at, Vec::new()));
        }
        let Some(r) = self.ast.types.as_record(q) else { return };
        if self.ast.decls.get(r).flags().contains(kiln_ast::decl::DeclFlags::INVALID) {
            return;
        }
        if let Some(def) = self.ast.definition(r) {
            for b in self.ast.decls.bases(def, &self.ast.lists) {
                if b.is_virtual {
                    out.push(note(
                        DiagId::note_unsatisfied_trait_vbase,
                        at,
                        vec![self.ty_arg(b.ty)],
                    ));
                }
                if !self.is_trivially_copyable_type(b.ty) {
                    out.push(note(
                        DiagId::note_unsatisfied_trait_ntc_base,
                        at,
                        vec![self.ty_arg(b.ty)],
                    ));
                }
            }
            for m in self.ast.members(def) {
                if self.kind_of(m) != DeclKind::Field {
                    continue;
                }
                let ft = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                if !self.is_trivially_copyable_type(ft) {
                    out.push(note(
                        DiagId::note_unsatisfied_trait_ntc_field,
                        at,
                        vec![self.decl_name(m), self.ty_arg(ft)],
                    ));
                }
            }
            let methods: Vec<DeclId> = self
                .ast
                .members(def)
                .filter(|&m| {
                    matches!(
                        self.kind_of(m),
                        DeclKind::CXXMethod | DeclKind::CXXConstructor | DeclKind::CXXDestructor
                    ) && self.is_user_provided(m)
                })
                .collect();
            if methods.iter().any(|&m| self.kind_of(m) == DeclKind::CXXDestructor) {
                out.push(note(DiagId::note_unsatisfied_trait_user_dtor, at, Vec::new()));
            }
            for m in methods {
                let (assign, mv) = match self.special_kind_of(m) {
                    Some(SpecialKind::CopyCtor) => (0, 0),
                    Some(SpecialKind::MoveCtor) => (0, 1),
                    Some(SpecialKind::CopyAssign) => (1, 0),
                    Some(SpecialKind::MoveAssign) => (1, 1),
                    _ => continue,
                };
                out.push(note(
                    DiagId::note_unsatisfied_trait_user_special,
                    at,
                    vec![mv.to_string(), assign.to_string()],
                ));
            }
        }
        let rt = self.tag_type(r);
        out.push(note(
            DiagId::note_entity_defined_here,
            self.ast.decls.get(r).loc(),
            vec![self.ty_arg(rt)],
        ));
    }
}

/// The trait `std::name` computes, among those explained.
fn std_trait(name: &str) -> Option<TypeTrait> {
    match name {
        "is_trivially_copyable" => Some(TypeTrait::IsTriviallyCopyable),
        _ => None,
    }
}
