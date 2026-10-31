//! Overload resolution (spec/sema-overload.md): candidate sets, implicit
//! conversion sequences, the best viable function, and their
//! diagnostics.
//!
//! Every overloaded context runs `begin_set` → `add_*` → `best_viable` →
//! perform or diagnose → `end_set` (§1.2). Sets come from a pool on
//! [`Sema`] so steady-state resolution allocates nothing (§1.4).

pub(crate) mod address;
pub(crate) mod adl;
pub(crate) mod alloc;
pub(crate) mod arrow;
pub(crate) mod best;
pub(crate) mod builtin;
pub(crate) mod call;
pub(crate) mod diag;
pub(crate) mod ics;
pub(crate) mod member;
pub(crate) mod oper;
pub(crate) mod redecl;
pub(crate) mod surrogate;
pub(crate) mod udc;

use std::ops::Range;

use kiln_ast::types::RefQual;
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;

pub(crate) use ics::{ArgInfo, BadKind, Ics, IcsOptions};

use crate::Sema;
use crate::conv::{RefBinding, StandardConversion};

/// What a candidate set resolves (spec/sema-overload.md §10.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SetKind {
    /// A call ([over.match.call]) or constructor call ([over.match.ctor]).
    #[default]
    Normal,
    /// [over.match.copy]: copy-initialization of a class by a
    /// user-defined conversion.
    CopyInit,
    /// [over.match.conv]: a non-class object from a class.
    ConvToNonClass,
    /// [over.match.ref]: a reference bound to the result of a conversion
    /// function.
    ConvToRef {
        /// The reference is an lvalue reference.
        lvalue: bool,
    },
    /// [over.match.list] phase 1 (initializer-list constructors) or 2.
    ListCtor {
        /// 1 or 2.
        phase: u8,
    },
    /// [over.over]: the address of an overloaded function. WP-068a
    /// resolves non-template targets without a set (`address.rs`); the
    /// kind is for WP-071's template candidates.
    #[allow(dead_code, reason = "WP-071: [over.over] with function templates")]
    Address,
}

/// The kind of a candidate (spec/sema-overload.md §10.1; `Surrogate`,
/// `Builtin` and `DeductionGuide` are WP-068b and S3T).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CandidateKind {
    /// A non-member function (or a static member called without an
    /// object).
    Function,
    /// A member function with an object argument.
    Method,
    /// A constructor.
    Ctor,
    /// A conversion function ([over.match.copy], [over.match.conv],
    /// [over.match.ref]).
    ConvFn,
    /// A built-in operator ([over.built]): `decl` is `None` and the
    /// parameter types are [`Candidate::builtin`].
    Builtin,
    /// A surrogate call function ([over.call.object]/2): `decl` is the
    /// conversion function to a pointer or reference to function.
    Surrogate,
}

/// Why a candidate is not viable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum CandidateFailure {
    /// Viable.
    #[default]
    None,
    /// More arguments than parameters (and not variadic).
    TooManyArgs {
        /// The parameter count.
        max: u16,
    },
    /// Fewer arguments than parameters without defaults.
    TooFewArgs {
        /// The parameters without a default argument.
        min: u16,
    },
    /// Argument `arg` (0-based; `u16::MAX` the object) has no implicit
    /// conversion sequence.
    BadConversion {
        /// The argument.
        arg: u16,
    },
    /// Template argument deduction failed (WP-071).
    BadDeduction,
    /// The trailing requires-clause is not satisfied (WP-073).
    #[allow(dead_code, reason = "stub: WP-073 constraints")]
    ConstraintsNotSatisfied,
    /// An `explicit` constructor or conversion function in
    /// copy-initialization ([over.match.copy]/1.1).
    Explicit,
    /// The result of a conversion function does not convert to the
    /// target ([over.match.conv]/1).
    BadFinalConversion,
}

/// A rewritten comparison candidate's form ([over.match.oper]/3.4;
/// clang `OverloadCandidateRewriteKind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Rewrite {
    /// Not rewritten.
    #[default]
    None,
    /// `y @ x` for `x @ y` (a reversed `==` or `<=>`).
    Reversed,
    /// A different operator: `==` for `!=`, `<=>` for a relational
    /// operator.
    Different,
    /// A different operator with the operands reversed.
    DifferentReversed,
}

impl Rewrite {
    /// The parameters take the operands in reverse order.
    pub(crate) fn is_reversed(self) -> bool {
        matches!(self, Rewrite::Reversed | Rewrite::DifferentReversed)
    }
}

/// One candidate function (spec/sema-overload.md §10.1).
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    /// The function (`None` for built-in candidates, WP-068b).
    pub decl: Option<DeclId>,
    /// The declaration lookup found (a `UsingShadowDecl`, or `decl`).
    pub found: DeclId,
    /// The function template a specialization came from (WP-071).
    pub template: Option<DeclId>,
    /// The candidate's kind.
    pub kind: CandidateKind,
    /// The candidate's conversion sequences in the set's `ics`: the
    /// object slot first when the set has an object argument.
    pub ics: Range<u32>,
    /// Viable ([over.match.viable]).
    pub viable: bool,
    /// Why not, when not viable.
    pub failure: CandidateFailure,
    /// The rewritten form ([over.match.oper]/3.4).
    pub rewrite: Rewrite,
    /// Found only by argument-dependent lookup.
    pub adl_only: bool,
    /// One of the candidates an ambiguity is reported between.
    pub ambiguous_mark: bool,
    /// [over.match.best]/2.2: the conversion of the result of a
    /// conversion function to the destination.
    pub final_conv: Option<StandardConversion>,
    /// A built-in candidate's parameter types (the second is
    /// [`QualType::ERROR`] for a unary operator).
    pub builtin: [QualType; 2],
    /// `BadDeduction`: why the template's deduction failed (§10.4).
    pub deduction: Option<Box<crate::templ::deduce_call::DeductionFailure>>,
}

/// The result of overload resolution (spec/sema-overload.md §10.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OvlOutcome {
    /// Candidate `.0` is the best viable function.
    Success(u32),
    /// No candidate is viable.
    NoViable,
    /// No viable candidate is better than all others.
    Ambiguous,
    /// The best viable function, `.0`, is deleted.
    Deleted(u32),
}

impl Candidate {
    /// A viable candidate with no conversion sequences yet.
    pub(crate) fn new(f: DeclId, found: DeclId, kind: CandidateKind) -> Candidate {
        Candidate {
            decl: Some(f),
            found,
            template: None,
            kind,
            ics: 0..0,
            viable: true,
            failure: CandidateFailure::None,
            rewrite: Rewrite::None,
            adl_only: false,
            ambiguous_mark: false,
            final_conv: None,
            builtin: [QualType::ERROR; 2],
            deduction: None,
        }
    }

    /// A viable built-in candidate with parameter types `params`.
    pub(crate) fn builtin(params: [QualType; 2]) -> Candidate {
        Candidate {
            decl: None,
            kind: CandidateKind::Builtin,
            builtin: params,
            ..Candidate::new(DeclId::new(1), DeclId::new(1), CandidateKind::Builtin)
        }
    }

    /// The function (meaningless for a built-in candidate, whose `decl`
    /// is `None`).
    pub(crate) fn func(&self) -> DeclId {
        self.decl.unwrap_or(self.found)
    }
}

/// A set of candidate functions for one resolution
/// (spec/sema-overload.md §10.1). Pooled: see [`OverloadScratch`].
#[derive(Clone, Debug)]
pub(crate) struct OverloadCandidateSet {
    /// What is resolved.
    pub kind: SetKind,
    /// Where (the primary diagnostic's location).
    pub loc: Loc,
    /// The arguments, once.
    pub args: Vec<ArgInfo>,
    /// The object argument of a member call or conversion.
    pub object: Option<ArgInfo>,
    /// The candidates in insertion (= note) order.
    pub cands: Vec<Candidate>,
    /// The candidates' conversion sequences, indexed by `Candidate::ics`.
    pub ics: Vec<Ics>,
    /// The target of a conversion ([over.match.copy], [over.match.conv],
    /// [over.match.ref]) or address resolution.
    pub target: QualType,
    /// A lookup buffer, also used for de-duplication.
    pub found: Vec<DeclId>,
    /// Parameter types of the candidate being added.
    pub params: Vec<QualType>,
    /// Function templates were found but not added (deduction is
    /// WP-071).
    pub skipped_templates: bool,
    /// A candidate has a *requires-clause*, which is not checked yet
    /// (WP-073): the outcome is uncertain.
    pub uncertain: bool,
    /// An operator set ([over.match.oper]): `object` is the first operand,
    /// which a non-member candidate takes as its first parameter.
    pub oper: bool,
    /// The number of arguments clang's candidate notes count
    /// (`CompareOverloadCandidatesForDisplay::NumArgs`) beyond `args`:
    /// 1 for an operator set other than a call, whose argument list
    /// includes the first operand.
    pub note_extra_args: usize,
    /// The operator of the built-in candidates, for their notes.
    pub builtin_op: Option<kiln_ast::names::OverloadedOperator>,
    /// Pairs `(c1, c2)` of viable function template specializations
    /// whose template is more specialized for `c1`
    /// ([over.match.best.general]/2.5), precomputed by
    /// `order_template_candidates` because `better` cannot deduce.
    pub tmpl_order: Vec<(u32, u32)>,
}

impl Default for OverloadCandidateSet {
    fn default() -> Self {
        OverloadCandidateSet {
            kind: SetKind::Normal,
            loc: Loc::INVALID,
            args: Vec::new(),
            object: None,
            cands: Vec::new(),
            ics: Vec::new(),
            target: QualType::ERROR,
            found: Vec::new(),
            params: Vec::new(),
            skipped_templates: false,
            uncertain: false,
            oper: false,
            note_extra_args: 0,
            builtin_op: None,
            tmpl_order: Vec::new(),
        }
    }
}

impl OverloadCandidateSet {
    /// Empties the set, keeping its allocations.
    pub(crate) fn clear(&mut self) {
        self.kind = SetKind::Normal;
        self.loc = Loc::INVALID;
        self.args.clear();
        self.object = None;
        self.cands.clear();
        self.ics.clear();
        self.target = QualType::ERROR;
        self.found.clear();
        self.params.clear();
        self.skipped_templates = false;
        self.uncertain = false;
        self.oper = false;
        self.note_extra_args = 0;
        self.builtin_op = None;
        self.tmpl_order.clear();
    }

    /// The conversion sequences of candidate `c`.
    pub(crate) fn ics_of(&self, c: u32) -> &[Ics] {
        let r = &self.cands[c as usize].ics;
        &self.ics[r.start as usize..r.end as usize]
    }

    /// The indices of the viable candidates.
    pub(crate) fn viable(&self) -> impl Iterator<Item = u32> + '_ {
        self.cands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.viable)
            .map(|(i, _)| u32::try_from(i).unwrap_or(u32::MAX))
    }

    /// Candidate `c`.
    pub(crate) fn cand(&self, c: u32) -> &Candidate {
        &self.cands[c as usize]
    }
}

/// Bench counters (spec/bench.md `ovl.*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverloadStats {
    /// Candidate sets resolved.
    pub sets: u64,
    /// Candidates added.
    pub candidates: u64,
    /// Implicit conversion sequences computed.
    pub ics: u64,
    /// Built-in candidates added (WP-068b).
    pub builtin_candidates: u64,
    /// The largest set.
    pub max_set: u64,
}

/// The pool of candidate sets (spec/sema-overload.md §1.4): nested
/// resolutions take one each; pool depth = nesting depth.
#[derive(Debug, Default)]
pub(crate) struct OverloadScratch {
    pool: Vec<OverloadCandidateSet>,
    /// Braced-init-list arguments being resolved (an [`ArgInfo::list`]
    /// indexes here).
    pub lists: Vec<crate::init::InitList>,
    /// Counters.
    pub stats: OverloadStats,
    /// The set of the last constructor selection that failed
    /// (`Missing`/`Ambiguous`), kept for its diagnostic notes.
    pub failed: Option<OverloadCandidateSet>,
    /// What lookup found for an `UnresolvedLookupExpr` callee beyond its
    /// decls, kept only when there is something to keep (a using-shadow
    /// or a naming class): consumed by [`Sema::resolve_unresolved_call`].
    pub ule_found: kiln_source::fx::FxHashMap<kiln_ast::ExprId, UleFound>,
    /// The overload set an overloaded member access (a `MemberExpr` of
    /// type [`QualType::OVERLOAD`], kiln's `UnresolvedMemberExpr`) names:
    /// consumed by [`Sema::resolve_member_call`].
    pub members: kiln_source::fx::FxHashMap<kiln_ast::ExprId, OvlMember>,
    /// How many user-defined conversions are being applied
    /// ([`Sema::user_convert`]).
    pub udc_depth: u8,
    /// The `DeclRefExpr` a one-candidate call through an
    /// argument-dependent name resolved to (clang's
    /// `UnresolvedLookupExpr` callee): its argument errors are
    /// overload-resolution ones ("no matching function"), where a name
    /// without ADL (qualified, parenthesized, block-scope) reports the
    /// conversion directly. Consumed by [`Sema::act_on_call`].
    pub adl_single: Option<kiln_ast::ExprId>,
    /// The conversion targets of complete classes, for built-in operator
    /// candidates (`sema.ovl.conv_types`).
    pub conv_types: kiln_source::fx::FxHashMap<DeclId, builtin::ConvTargets>,
    /// One built-in resolution's conversion sequences, by (operand,
    /// parameter type, [`IcsOptions`] bits).
    pub builtin_memo: kiln_source::fx::FxHashMap<(u8, QualType, u8), Ics>,
}

/// An overloaded member access `b.f` / `b->f` / implicit `this->f`
/// (spec/sema-overload.md §2.3).
#[derive(Clone, Debug)]
pub(crate) struct OvlMember {
    /// The member name.
    pub name: kiln_source::symbol::Symbol,
    /// The member functions (and member function templates), in
    /// declaration order.
    pub decls: Vec<DeclId>,
    /// Parallel to `decls`: the declaration lookup found.
    pub found: Vec<DeclId>,
    /// The naming class ([class.access.base]/5).
    pub naming: DeclId,
    /// The object expression's class type (the pointee for `->`).
    pub obj_ty: QualType,
    /// The object's complete class.
    pub def: DeclId,
    /// The object expression (a pointer for `arrow`).
    pub base: kiln_ast::ExprId,
    /// `->` (or an implicit `this->`).
    pub arrow: bool,
    /// The nested-name-specifier of a qualified-id (`C::f(…)`: no virtual
    /// dispatch).
    pub nns: Option<kiln_ast::NnsId>,
    /// The object is an implicit `this` (a static winner is then named
    /// by a `DeclRefExpr`, as clang does).
    pub implicit: bool,
}

/// The lookup details of an `UnresolvedLookupExpr` its decl list does not
/// record.
#[derive(Clone, Debug, Default)]
pub(crate) struct UleFound {
    /// Parallel to the ULE's decls: the declaration lookup found (clang's
    /// `FoundDecl`).
    pub found: Vec<DeclId>,
    /// The naming class ([class.access.base]/5) of a class member lookup.
    pub naming: Option<DeclId>,
}

impl OverloadScratch {
    /// The heap bytes the pool holds (spec/sema-overload.md §1.4).
    pub(crate) fn heap_size(&self) -> usize {
        let one = |s: &OverloadCandidateSet| {
            s.args.capacity() * size_of::<ArgInfo>()
                + s.cands.capacity() * size_of::<Candidate>()
                + s.ics.capacity() * size_of::<Ics>()
                + s.found.capacity() * size_of::<DeclId>()
                + s.params.capacity() * size_of::<QualType>()
        };
        self.pool.capacity() * size_of::<OverloadCandidateSet>()
            + self.pool.iter().map(one).sum::<usize>()
            + self.failed.as_ref().map_or(0, one)
            + self.ule_found.capacity() * size_of::<(kiln_ast::ExprId, UleFound)>()
            + self
                .ule_found
                .values()
                .map(|u| u.found.capacity() * size_of::<DeclId>())
                .sum::<usize>()
            + self.members.capacity() * size_of::<(kiln_ast::ExprId, OvlMember)>()
            + self
                .members
                .values()
                .map(|m| (m.decls.capacity() + m.found.capacity()) * size_of::<DeclId>())
                .sum::<usize>()
            + self.builtin_memo.capacity() * size_of::<((u8, QualType, u8), Ics)>()
    }

    /// The heap bytes of the conversion-target cache.
    pub(crate) fn conv_types_size(&self) -> usize {
        self.conv_types.capacity() * size_of::<(DeclId, builtin::ConvTargets)>()
            + self
                .conv_types
                .values()
                .map(|c| c.targets.capacity() * size_of::<(QualType, bool)>())
                .sum::<usize>()
    }
}

impl Sema<'_, '_> {
    /// A cleared candidate set of kind `kind` at `loc` (from the pool).
    pub(crate) fn begin_set(&mut self, kind: SetKind, loc: Loc) -> OverloadCandidateSet {
        let mut s = self.ovl.pool.pop().unwrap_or_default();
        s.clear();
        s.kind = kind;
        s.loc = loc;
        self.ovl.stats.sets += 1;
        s
    }

    /// Returns `set` to the pool.
    pub(crate) fn end_set(&mut self, set: OverloadCandidateSet) {
        let n = set.cands.len() as u64;
        if n > self.ovl.stats.max_set {
            self.ovl.stats.max_set = n;
        }
        self.ovl.pool.push(set);
    }

    /// The overload-resolution counters so far.
    pub fn overload_stats(&self) -> OverloadStats {
        self.ovl.stats
    }

    /// The memory overload resolution holds (spec/sema-overload.md §1.4):
    /// the row `sema.ovl.pool`, whose count is the pooled candidate sets
    /// (the deepest nesting so far) and whose bytes are the pool's heap.
    pub fn overload_memory(&self) -> kiln_ast::mem::MemReport {
        let bytes = self.ovl.heap_size() as u64;
        let mut r = kiln_ast::mem::MemReport::new();
        r.push(kiln_ast::mem::MemRow::new(
            "sema.ovl.pool",
            self.ovl.pool.len() as u64,
            bytes,
            bytes,
        ));
        let bytes = self.ovl.conv_types_size() as u64;
        r.push(kiln_ast::mem::MemRow::new(
            "sema.ovl.conv_types",
            self.ovl.conv_types.len() as u64,
            bytes,
            bytes,
        ));
        r
    }

    /// The parameter types of function `f` (its declared type's), into
    /// `out`; and whether it is variadic.
    pub(crate) fn fn_param_types(&self, f: DeclId, out: &mut Vec<QualType>) -> bool {
        out.clear();
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        match self.ast.types.proto(ty) {
            Some(p) => {
                let v = p.info().variadic();
                out.extend_from_slice(self.ast.types.params(p));
                v
            }
            None => false,
        }
    }

    /// The minimum argument count of `f` ([dcl.fct.default]): parameters
    /// up to the last without a default argument.
    pub(crate) fn fn_min_args(&self, f: DeclId) -> usize {
        let Some(fd) = self.ast.decls.function(f) else { return 0 };
        let n = self.ast.lists.get(fd.params).len();
        let mut min = 0;
        for i in 0..n {
            let Some(p) = self.ast.lists.at::<DeclId>(fd.params, i) else { break };
            if !self.param_has_default(p) {
                min = i + 1;
            }
        }
        min
    }

    /// The object binding of method `m`: its cv-qualifiers and
    /// ref-qualifier.
    pub(crate) fn method_object_quals(&self, m: DeclId) -> (kiln_ast::types::Quals, RefQual) {
        let ty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
        let info =
            self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::info).unwrap_or_default();
        (info.this_quals(), info.ref_qual())
    }

    /// Whether `m` is a member function with an explicit object
    /// parameter ([dcl.fct]/6).
    pub(crate) fn has_explicit_object(&self, m: DeclId) -> bool {
        self.ast.decls.function(m).is_some_and(|f| f.spec.has_explicit_object())
    }

    /// Whether `f` is deleted, explicitly or as a defaulted special
    /// member ([dcl.fct.def.delete], [class.default.ctor]/2, ...).
    pub(crate) fn ovl_deleted(&self, f: DeclId) -> bool {
        self.is_deleted_fn(f) || self.fn_spec(f).is_deleted()
    }
}

/// The identity binding of a static member's object slot
/// ([over.match.funcs]/4): never compared.
pub(crate) fn ignored_object(ty: QualType) -> Ics {
    Ics::Standard(StandardConversion::identity(ty, ty, RefBinding::IGNORED))
}
