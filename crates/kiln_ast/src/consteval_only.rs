//! Consteval-only values and immediate objects (P4101R1, the working
//! draft's [expr.const.const]/1-2; spec/reflection-repr.md §6).
//!
//! A *consteval-only value* is a reflection other than the null
//! reflection, or a pointer or pointer-to-member that points to an
//! immediate function or to or past the end of an immediate object. An
//! object is an *immediate object* when its complete object has a
//! constituent value that is consteval-only or a constituent reference
//! that refers to an immediate object or immediate function. Both are
//! properties of frozen values ([`ConstValueId`]), never of types: sema
//! decides with them which expressions are immediate constant expressions
//! and which variables may not be non-`constexpr`, and codegen which
//! variables have no run-time object. This module is that one predicate,
//! shared by both.

use crate::ast::Ast;
use crate::consts::{ConstValue, PtrBase, Scalar};
use crate::decl::{DeclKind, VarEvalState};
use crate::ids::{ConstValueId, DeclId, ReflectionId};
use kiln_source::fx::FxHashSet;

impl Ast {
    /// Whether frozen value `c` has a consteval-only constituent value or
    /// a constituent reference to an immediate object or function
    /// ([expr.const.const]/1-2, P4101R1): the object it is the value of
    /// is an immediate object. For the value of a reference variable
    /// (the pointer to the object it binds) this is whether the reference
    /// refers to an immediate object or function.
    pub fn has_consteval_only_value(&self, c: ConstValueId) -> bool {
        let mut seen = Seen::default();
        self.value_is_immediate(c, &mut seen)
    }

    /// Whether variable or template parameter object `d` is an immediate
    /// object (or, for a reference, refers to one): its constant value,
    /// if it has one, has a consteval-only constituent
    /// ([`Ast::has_consteval_only_value`]). False for a variable not
    /// (yet) evaluated to a constant.
    pub fn is_immediate_var(&self, d: DeclId) -> bool {
        let mut seen = Seen::default();
        self.decl_is_immediate(d, &mut seen)
    }

    /// Whether frozen value `c` holds a reflection (the null one too) as
    /// a subobject, not through a pointer: an object of it has no
    /// run-time representation in codegen yet (spec/codegen.md
    /// "Consteval-only variables").
    pub fn holds_reflection(&self, c: ConstValueId) -> bool {
        let mut stack = vec![c];
        let mut steps = 0usize;
        while let Some(c) = stack.pop() {
            steps += 1;
            if steps > 1_000_000 {
                break;
            }
            match self.consts.get(c) {
                ConstValue::Scalar(Scalar::Reflection { .. }) => return true,
                ConstValue::Scalar(_) => {}
                ConstValue::Aggregate(img) => stack.extend(img.relocs.iter().map(|r| r.value)),
            }
        }
        false
    }

    /// The constant value of variable, temporary or template parameter
    /// object `d`, through its redeclarations.
    pub fn object_value(&self, d: DeclId) -> Option<ConstValueId> {
        match self.decls.get(d).kind() {
            DeclKind::TemplateParamObject => self.templates.param_object_value(d),
            DeclKind::LifetimeExtendedTemporary => {
                self.decls.lifetime_extended(d).and_then(|l| l.value)
            }
            DeclKind::Var => {
                self.redecls_of(d).find_map(|x| match self.decls.var(x)?.eval.state() {
                    VarEvalState::Value(c) => Some(c),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    fn decl_is_immediate(&self, d: DeclId, seen: &mut Seen) -> bool {
        if !seen.decls.insert(d) {
            return false;
        }
        self.object_value(d).is_some_and(|c| self.value_is_immediate(c, seen))
    }

    fn value_is_immediate(&self, c: ConstValueId, seen: &mut Seen) -> bool {
        match self.consts.get(c) {
            ConstValue::Scalar(s) => self.scalar_is_immediate(s, seen),
            // Only an aggregate (or a declaration) can be reached again.
            ConstValue::Aggregate(_) if !seen.values.insert(c) => false,
            ConstValue::Aggregate(img) => {
                let relocs: Vec<ConstValueId> = img.relocs.iter().map(|r| r.value).collect();
                relocs.into_iter().any(|v| self.value_is_immediate(v, seen))
            }
        }
    }

    fn scalar_is_immediate(&self, s: Scalar<'_>, seen: &mut Seen) -> bool {
        match s {
            Scalar::Reflection { id, .. } => id != ReflectionId::NULL,
            Scalar::Pointer { base, .. } => match base {
                PtrBase::Function(f) => self.fn_is_immediate(f),
                PtrBase::Decl(d) | PtrBase::Temporary(d) => self.decl_is_immediate(d, seen),
                PtrBase::Const(v) => self.value_is_immediate(v, seen),
                _ => false,
            },
            Scalar::MemberPtr { member, .. } => self.fn_is_immediate(member),
            _ => false,
        }
    }

    /// Whether `f` is an immediate function (`consteval`, or escalated).
    fn fn_is_immediate(&self, f: DeclId) -> bool {
        self.decls.function(f).is_some_and(|x| x.spec.is_immediate())
    }
}

/// The values and declarations already walked (a constant may point to
/// itself: `struct N { const N* p; } constexpr n{&n};`).
#[derive(Default)]
struct Seen {
    values: FxHashSet<ConstValueId>,
    decls: FxHashSet<DeclId>,
}
