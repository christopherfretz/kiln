//! Concepts and constraints (spec/sema-templates.md §8; WP-073):
//! normalization and atomic-constraint identity ([`normal`]),
//! satisfaction with its caches and note chains ([`sat`]),
//! requires-expressions ([`requires`]) and subsumption ([`subsume`]).

pub mod normal;
pub mod requires;
pub mod sat;
pub mod subsume;

use kiln_ast::ids::ArgListId;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

pub use normal::{NormId, NormNode, NormTable};

/// A recorded satisfaction (index into [`ConceptState::sats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SatId(u32);

/// The result of checking a constraint (§8.3): `records` explain a
/// failure, in clang's note order.
#[derive(Clone, Debug, Default)]
pub struct Satisfaction {
    /// Satisfied.
    pub ok: bool,
    /// Why not (empty when `ok`).
    pub records: Vec<SatRecord>,
}

/// One note of a §8.7 chain. Notes whose text starts with
/// "because"/"and" take the position flag as their first argument
/// (`select`); `nested` is a concept's own satisfaction printed after it.
#[derive(Clone, Debug)]
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
#[derive(Clone, Copy, Debug, Default)]
pub struct SatCounters {
    /// Satisfaction checks computed.
    pub count: u64,
    /// Satisfaction checks answered from the cache.
    pub cache_hits: u64,
    /// Subsumption checks computed.
    pub subsume: u64,
}

/// The concept engine's state (§11.2).
#[derive(Debug, Default)]
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
}
