//! Surrogate call functions ([over.call.object]/2; clang
//! `AddSurrogateCandidate`, `NoteSurrogateCandidate`): a call `x(args)`
//! on a class object also considers each non-explicit conversion
//! function of `x` to a pointer or reference to function `R(P...)` as a
//! candidate `R f(conv-type, P...)`.

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{FuncProto, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::ics::{UdcVia, UserConv};
use super::{Candidate, CandidateFailure, CandidateKind, Ics, IcsOptions, OverloadCandidateSet};
use crate::Sema;
use crate::conv::StandardConversion;

impl Sema<'_, '_> {
    /// The function type a surrogate for conversion function `conv`
    /// calls: its result type without a reference and then a pointer.
    pub(crate) fn surrogate_proto(&self, conv: DeclId) -> Option<FuncProto> {
        let t = self.conversion_result_type(conv);
        let t = self.non_reference(t);
        let t = if self.ast.types.is_pointer(t) { self.ast.types.pointee(t)? } else { t };
        self.ast.types.proto(t).cloned()
    }

    /// Adds the surrogate call functions of the object `obj` (the set's
    /// object argument) after its `operator()` candidates.
    pub(crate) fn add_surrogates(&mut self, set: &mut OverloadCandidateSet, obj: ExprId) {
        let t = self.ety(obj);
        for (f, found) in self.conversion_functions(t) {
            if self.kind_of(f) != DeclKind::CXXConversion || self.fn_spec(f).is_explicit() {
                continue;
            }
            let Some(proto) = self.surrogate_proto(f) else { continue };
            self.add_surrogate(set, f, found, &proto);
        }
    }

    fn add_surrogate(
        &mut self,
        set: &mut OverloadCandidateSet,
        f: DeclId,
        found: DeclId,
        proto: &FuncProto,
    ) {
        self.ovl.stats.candidates += 1;
        let mut cand = Candidate::new(f, found, CandidateKind::Surrogate);
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        let Some(o) = set.object else { return };
        let obj = self.object_ics(&o, f);
        match obj {
            Ics::Standard(before) => {
                // A user-defined conversion whose first step is the object
                // binding (clang records it so).
                let to = before.to_types[2];
                let after = StandardConversion::identity(to, to, before.rb);
                set.ics.push(Ics::UserDefined(UserConv {
                    before,
                    func: Some(f),
                    found: Some(found),
                    after,
                    via: UdcVia::ConvFn,
                    list_ctor: false,
                }));
            }
            bad => {
                set.ics.push(bad);
                cand.viable = false;
                cand.failure = CandidateFailure::BadConversion { arg: u16::MAX };
            }
        }
        let params: Vec<QualType> = self.ast.types.params(proto).to_vec();
        let variadic = proto.info().variadic();
        let nargs = set.args.len();
        if !cand.viable {
        } else if nargs > params.len() && !variadic {
            cand.viable = false;
            cand.failure =
                CandidateFailure::TooManyArgs { max: u16::try_from(params.len()).unwrap_or(u16::MAX) };
        } else if nargs < params.len() {
            cand.viable = false;
            cand.failure =
                CandidateFailure::TooFewArgs { min: u16::try_from(params.len()).unwrap_or(u16::MAX) };
        } else {
            for i in 0..nargs {
                let a = set.args[i];
                let x = match params.get(i) {
                    Some(&p) => self.try_implicit_conversion(&a, p, IcsOptions::NONE),
                    None => Ics::Ellipsis,
                };
                set.ics.push(x);
                if x.is_bad() {
                    cand.viable = false;
                    cand.failure =
                        CandidateFailure::BadConversion { arg: u16::try_from(i).unwrap_or(u16::MAX) };
                    break;
                }
            }
        }
        cand.ics = start..u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        set.cands.push(cand);
    }

    /// clang `NoteSurrogateCandidate`: "conversion candidate of type
    /// 'R (*)(P...)'", the function type desugared.
    pub(crate) fn surrogate_note(&mut self, conv: DeclId) -> Diag {
        let t = self.conversion_result_type(conv);
        let (lref, rref) = match self.ast.types.canonical_view(t) {
            TypeView::LValueRef(_) => (true, false),
            TypeView::RValueRef(_) => (false, true),
            _ => (false, false),
        };
        let t = self.non_reference(t);
        let ptr = self.ast.types.is_pointer(t);
        let t = if ptr { self.ast.types.pointee(t).unwrap_or(t) } else { t };
        let mut fty = self.ast.types.canonical(t);
        if ptr {
            fty = self.ast.types.pointer_to(fty);
        }
        if rref {
            fty = self.ast.types.rvalue_ref_to(fty);
        }
        if lref {
            fty = self.ast.types.lvalue_ref_to(fty);
        }
        let at = self.ast.decls.get(conv).loc();
        self.diag(DiagId::note_ovl_surrogate_candidate, at).arg(self.ty_arg(fty))
    }

    /// `obj(args)` through the surrogate for conversion function `f`:
    /// the call of the converted object (clang
    /// `BuildCallToObjectOfClassType`).
    pub(crate) fn call_surrogate(
        &mut self,
        obj: ExprId,
        f: DeclId,
        found: DeclId,
        args: &[ExprId],
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let callee = self.call_conversion_function(obj, f, found, loc);
        if self.is_err_expr(callee) {
            return self.recover_err(args, loc);
        }
        self.act_on_call(callee, args, loc, end)
    }
}
