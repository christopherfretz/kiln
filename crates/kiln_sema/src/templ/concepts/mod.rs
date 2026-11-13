//! Concepts and constraints (spec/sema-templates.md §8; WP-073):
//! normalization and atomic-constraint identity ([`normal`]),
//! satisfaction with its caches and note chains ([`sat`]),
//! requires-expressions ([`requires`]) and subsumption ([`subsume`]).

pub mod normal;
pub mod requires;
pub mod sat;
pub mod subsume;
mod used;

use kiln_ast::ids::ArgListId;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

pub use normal::{NormId, NormNode, NormTable};

/// A recorded satisfaction (index into [`ConceptState::sats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub struct SatId(u32);

/// The result of checking a constraint (§8.3): `records` explain a
/// failure, in clang's note order.
#[derive(Clone, Debug, Default, kiln_snap::Snap)]
pub struct Satisfaction {
    /// Satisfied.
    pub ok: bool,
    /// Why not (empty when `ok`).
    pub records: Vec<SatRecord>,
}

/// One note of a §8.7 chain. Notes whose text starts with
/// "because"/"and" take the position flag as their first argument
/// (`select`); `nested` is a concept's own satisfaction printed after it.
#[derive(Clone, Debug, kiln_snap::Snap)]
pub struct SatRecord {
    /// The note.
    pub id: DiagId,
    /// Where.
    pub loc: Loc,
    /// The note takes the "and|because" selector first.
    pub select: bool,
    /// The remaining arguments.
    pub args: Vec<String>,
    /// The chain printed after this note.
    pub nested: Option<SatId>,
}

/// Counters for the instantiation report (§11.3).
#[derive(Clone, Copy, Debug, Default, kiln_snap::Snap)]
pub struct SatCounters {
    /// Satisfaction checks computed.
    pub count: u64,
    /// Satisfaction checks answered from the cache.
    pub cache_hits: u64,
    /// Subsumption checks computed.
    pub subsume: u64,
    /// Atomic constraints evaluated (§8.3 `atom_cache` misses).
    pub atoms: u64,
    /// Atomic constraints answered from the `atom_cache`.
    pub atom_hits: u64,
}

/// An `atom_cache` key: the atom and the canonical argument list of each
/// level.
pub(crate) type AtomKey = (ExprId, Vec<ArgListId>);

/// The concept engine's state (§11.2).
#[derive(Debug, Default, kiln_snap::Snap)]
pub struct ConceptState {
    pub(crate) sats: Vec<Satisfaction>,
    /// `(entity or concept, canonical args) -> SatId`.
    pub(crate) sat_cache: FxHashMap<(DeclId, ArgListId), SatId>,
    /// The satisfaction of a non-dependent `ConceptSpecializationExpr`
    /// or `RequiresExpr`, for the notes.
    pub(crate) expr_sat: FxHashMap<ExprId, SatId>,
    /// Concept-ids being checked (recursion, [temp.constr.normal]/1.4).
    pub(crate) checking: Vec<(DeclId, ArgListId)>,
    /// Normal forms (§8.2).
    pub(crate) norm: NormTable,
    pub(crate) normal_forms: FxHashMap<DeclId, Option<NormId>>,
    pub(crate) subsume_cache: FxHashMap<(NormId, NormId), bool>,
    /// The parameters of a depth an atom's expression names (§8.2,
    /// [temp.constr.atomic]/1), `None` when unknown.
    pub(crate) used_params: FxHashMap<(ExprId, u16), Option<u64>>,
    /// §8.3 `atom_cache`: (atom, canonical arguments) -> (satisfied, the
    /// records explaining a failure).
    pub(crate) atom_cache: FxHashMap<AtomKey, (bool, Vec<SatRecord>)>,
    /// A member of a class template specialization redeclared from a
    /// constrained member of the pattern (a member template, or a
    /// function with a trailing requires-clause): the pattern member and
    /// the specialization's levels. Its constraints stay the pattern's,
    /// unsubstituted, and are checked with these outer levels (clang
    /// keeps the pattern's clauses and checks them with the
    /// instantiation's `MultiLevelTemplateArgumentList`).
    pub(crate) member_constraints: FxHashMap<DeclId, (DeclId, crate::templ::args::MultiLevelArgs)>,
    /// Non-templated functions whose (ill-formed, diagnosed) trailing
    /// requires-clause is still checked with no template arguments
    /// ([dcl.decl.general]/5; clang keeps the clause and checks it
    /// during overload resolution and destructor selection).
    pub(crate) untemplated_requires: kiln_source::fx::FxHashSet<DeclId>,
    /// While the constraints of a class template partial specialization
    /// are checked for a match: `(partial, specialization)`. Its atoms
    /// check access from the specialization being matched, so a class
    /// that befriends `traits<It>` lets `requires It::private_flag` on
    /// `template <class I> struct traits<I>` see its members (clang
    /// accepts; libc++ 22's `__product_iterator_traits`, WP-446).
    pub(crate) partial_access: Option<(DeclId, DeclId)>,
    /// The type-constraint location of a variable or function declared
    /// with a constrained placeholder (`C auto x`, `C auto f()`).
    pub(crate) placeholder_locs: FxHashMap<DeclId, Loc>,
    /// Counters.
    pub counters: SatCounters,
}

impl ConceptState {
    /// The satisfaction `id`.
    pub fn sat(&self, id: SatId) -> &Satisfaction {
        &self.sats[id.0 as usize]
    }

    pub(crate) fn push(&mut self, s: Satisfaction) -> SatId {
        let id = SatId(u32::try_from(self.sats.len()).unwrap_or(u32::MAX));
        self.sats.push(s);
        id
    }
}

impl crate::Sema<'_, '_> {
    /// A complete requires-clause `e` ([temp.pre]/9): one naming a pack
    /// outside any expansion (`requires Cs<T>` with a concept template
    /// parameter pack `Cs`, `requires Int<Ts>`) is ill-formed
    /// ([temp.variadic]/12), diagnosed at its begin as "requires clause
    /// contains unexpanded parameter pack" (clang's
    /// `ActOnRequiresClause`) and replaced by an error expression.
    ///
    /// In a lambda's declarator (its template-head's requires-clause or
    /// its trailing one, before the body opens) the packs the lambda
    /// declares are diagnosed so, and an enclosing scope's make the
    /// lambda-expression contain an unexpanded pack, expanded by a pack
    /// expansion around it ([temp.variadic]/5; clang's
    /// `DiagnoseUnexpandedParameterPacks` with the lambda's `LocalPacks`).
    pub fn act_on_requires_clause(&mut self, e: ExprId) -> ExprId {
        let in_declarator = self.lambdas.last().is_some_and(|top| !top.body_open);
        let unexpanded = if in_declarator {
            self.diagnose_unexpanded_lambda_declarator(e, "requires clause")
        } else {
            self.diagnose_unexpanded_written(e, "requires clause", None)
        };
        if unexpanded {
            let at = self.begin_loc(e);
            return self.act_on_error_expr(&[e], at);
        }
        e
    }

    /// Checks a constraint-expression where it is written (a concept's
    /// definition or a requires-clause operand; clang's
    /// `CheckConstraintExpression`, [temp.constr.atomic]/3): each atomic
    /// constraint that is not type-dependent must have type `bool`.
    /// Diagnoses at the atom and returns false when one does not.
    pub fn check_constraint_expression(&mut self, e: ExprId) -> bool {
        self.check_constraint_at(e, 0)
    }

    fn check_constraint_at(&mut self, e: ExprId, depth: u32) -> bool {
        use kiln_ast::expr::{BinaryOp, ExprKind};
        let e = self.ignore_paren_imp_casts(e);
        let n = *self.ast.exprs.get(e);
        if depth < 256 {
            match n.kind() {
                ExprKind::BinaryOperator
                    if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
                {
                    let [l, r] = self.ast.exprs.operands(e);
                    return self.check_constraint_at(l, depth + 1)
                        && self.check_constraint_at(r, depth + 1);
                }
                ExprKind::ExprWithCleanups => {
                    let o = self.ast.exprs.operand(e);
                    return self.check_constraint_at(o, depth + 1);
                }
                ExprKind::CXXFoldExpr if !self.check_constraint_fold(e) => return false,
                _ => {}
            }
        }
        if self.expr_is_type_dependent(e) || self.has_errors_in(e) {
            return true;
        }
        let ty = self.ety(e);
        if self.is_error_type(ty)
            || self.ast.types.builtin(self.ast.types.canonical(ty).without_quals())
                == Some(kiln_ast::types::BuiltinType::Bool)
        {
            return true;
        }
        let arg = self.ty_arg(ty);
        let d = self.diag(DiagId::sema_non_bool_atomic_constraint, n.loc()).arg(arg);
        self.emit(d);
        false
    }

    /// [temp.constr.normal]/1.5.3: a fold whose pattern contains an
    /// unexpanded concept template parameter pack shall not contain an
    /// unexpanded template parameter pack of another kind (Example 5;
    /// kiln follows the wording, clang 22 accepts, spec/sema-templates.md
    /// §8.2). Diagnosed where the constraint is written.
    fn check_constraint_fold(&mut self, e: ExprId) -> bool {
        use crate::templ::pack::PackOperand;
        use kiln_ast::ids::RawId;
        use kiln_ast::templates::TtpKind;
        let n = *self.ast.exprs.get(e);
        let mut packs = Vec::new();
        for k in [n.a(), n.b()].into_iter().filter_map(ExprId::from_raw) {
            packs.extend(self.unexpanded_packs(PackOperand::Expr(k)));
        }
        let concept = packs.iter().filter(|&&p| self.ttp_kind(p) == TtpKind::Concept).count();
        if concept == 0 || concept == packs.len() {
            return true;
        }
        let d = self.diag(DiagId::sema_fold_constraint_mixed_packs, n.loc());
        self.emit(d);
        false
    }
}
