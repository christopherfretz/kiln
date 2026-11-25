//! The static-initialization fact of a variable (spec/ast.md §25,
//! WP-3650): opt-in (`-fkiln-static-init-facts`), off by default.
//!
//! For every non-dependent variable of static or thread storage duration
//! with a definition, sema records at the end of the translation unit
//! how the variable is initialized ([basic.start.static],
//! [basic.start.dynamic]) and whether its destruction needs a registered
//! destructor ([basic.start.term], constant destruction [expr.const]/7),
//! as code generation decides both: codegen reads the recorded fact
//! instead of deciding again (`kiln_codegen` `fold::prepass`), so the two
//! agree by construction. The fact lives in spare bits of
//! [`crate::decl::VarSpec`] (bits 16 to 23) of the declaration that holds
//! the initializer (else the definition): no node grows, and a snapshot
//! carries it with the specifiers. Read it with
//! [`crate::Ast::static_init_fact`].

/// How a variable of static or thread storage duration is initialized.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StaticInit {
    /// Constant-initialized ([basic.start.static]/2): no initializer that
    /// does anything at run time (none at all, a trivial default
    /// constructor, or zero-initialization only), or a constant
    /// initializer ([expr.const]/2). Its value is recorded as the
    /// variable's evaluated value ([`crate::decl::VarEvalState::Value`])
    /// when it has an initializer to evaluate.
    Constant,
    /// Not constant-initialized, but initialized statically all the same
    /// ([basic.start.static]/3): a scalar or initializer-list initializer
    /// that folds outside a constant context (clang's
    /// `ConstantEmitter::tryEmitPrivateForVarInit`). No dynamic
    /// initializer runs.
    EarlyStatic,
    /// Dynamic initialization ([basic.start.dynamic]): an initializer
    /// runs at program start (or on first use, for a static local, or
    /// per thread, for a `thread_local`).
    Dynamic,
    /// Not decided: the initializer was not evaluated, or its evaluation
    /// gave no verdict, for the reason given. Code generation decides on
    /// its own for such a variable.
    NotEvaluated(NotEvaluatedWhy),
}

/// Why a variable's initialization kind was not decided.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum NotEvaluatedWhy {
    /// The declaration is invalid.
    Invalid = 0,
    /// The initializer contains errors.
    InitErrors = 1,
    /// The evaluation exhausted the step budget (`-fconstexpr-steps=`).
    StepLimit = 2,
    /// The evaluation exhausted the call-depth budget
    /// (`-fconstexpr-depth=`).
    DepthLimit = 3,
    /// The evaluation exhausted the abstract-memory budget
    /// (`-fconstexpr-memory=`).
    MemoryLimit = 4,
    /// The native stack was too low to evaluate the initializer.
    Stack = 5,
    /// A GNU flexible array member is given elements: codegen folds the
    /// elements apart from the object.
    FlexibleArrayMember = 6,
    /// A static local whose initializer is not a constant initializer, in
    /// a function with a floating-point environment that a fold must not
    /// assume (`#pragma STDC FENV_ACCESS`, a dynamic rounding mode, or
    /// trapping exceptions, from a pragma or the command line).
    FpEnvironment = 7,
}

/// Whether the end of a variable's lifetime runs code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StaticDestruction {
    /// No destructor runs: the type is trivially destructible (or a
    /// reference whose initialization extends no temporary that needs
    /// one).
    Trivial,
    /// The type has a non-trivial destructor, but the variable has
    /// constant destruction ([expr.const]/7): no destructor is
    /// registered.
    Constant,
    /// A destructor is registered (`__cxa_atexit`, `__cxa_thread_atexit`
    /// for a `thread_local`): the variable's own, or for a reference that
    /// of a temporary its dynamic initialization extends
    /// ([class.temporary]/6). `[[clang::no_destroy]]` and
    /// `-fno-c++-static-destructors` still suppress the registration in
    /// code generation.
    Registered,
}

/// A variable's recorded static-initialization fact.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StaticInitFact {
    /// How it is initialized.
    pub init: StaticInit,
    /// How it is destroyed; `None` when that needs the initializer's value
    /// and its initialization was not decided.
    pub destruction: Option<StaticDestruction>,
}

/// The [`crate::decl::VarSpec`] bits holding the fact: bits 16 to 18 the
/// initialization (0 none recorded, 1 constant, 2 early static, 3
/// dynamic, 4 not evaluated), 19 to 21 the reason it was not evaluated,
/// 22 and 23 the destruction (0 undecided, 1 trivial, 2 constant, 3
/// registered).
pub(crate) const FACT_SHIFT: u32 = 16;
/// The mask of the fact bits.
pub(crate) const FACT_MASK: u32 = 0xff << FACT_SHIFT;

impl NotEvaluatedWhy {
    const ALL: [NotEvaluatedWhy; 8] = [
        NotEvaluatedWhy::Invalid,
        NotEvaluatedWhy::InitErrors,
        NotEvaluatedWhy::StepLimit,
        NotEvaluatedWhy::DepthLimit,
        NotEvaluatedWhy::MemoryLimit,
        NotEvaluatedWhy::Stack,
        NotEvaluatedWhy::FlexibleArrayMember,
        NotEvaluatedWhy::FpEnvironment,
    ];

    /// The dump's word.
    pub const fn name(self) -> &'static str {
        match self {
            NotEvaluatedWhy::Invalid => "invalid",
            NotEvaluatedWhy::InitErrors => "init-errors",
            NotEvaluatedWhy::StepLimit => "step-limit",
            NotEvaluatedWhy::DepthLimit => "depth-limit",
            NotEvaluatedWhy::MemoryLimit => "memory-limit",
            NotEvaluatedWhy::Stack => "stack",
            NotEvaluatedWhy::FlexibleArrayMember => "flexible-array-member",
            NotEvaluatedWhy::FpEnvironment => "fp-environment",
        }
    }
}

impl StaticInit {
    /// The dump's word (`static-init=<word>`).
    pub const fn name(self) -> &'static str {
        match self {
            StaticInit::Constant => "constant",
            StaticInit::EarlyStatic => "early-static",
            StaticInit::Dynamic => "dynamic",
            StaticInit::NotEvaluated(_) => "not-evaluated",
        }
    }
}

impl StaticDestruction {
    /// The dump's word (`static-destroy=<word>`).
    pub const fn name(self) -> &'static str {
        match self {
            StaticDestruction::Trivial => "trivial",
            StaticDestruction::Constant => "constant",
            StaticDestruction::Registered => "registered",
        }
    }
}

impl StaticInitFact {
    /// The fact's bits, in place ([`FACT_MASK`]).
    pub(crate) fn encode(self) -> u32 {
        let (init, why) = match self.init {
            StaticInit::Constant => (1, 0),
            StaticInit::EarlyStatic => (2, 0),
            StaticInit::Dynamic => (3, 0),
            StaticInit::NotEvaluated(w) => (4, w as u32),
        };
        let destruction = match self.destruction {
            None => 0,
            Some(StaticDestruction::Trivial) => 1,
            Some(StaticDestruction::Constant) => 2,
            Some(StaticDestruction::Registered) => 3,
        };
        (init | (why << 3) | (destruction << 6)) << FACT_SHIFT
    }

    /// The fact in `bits` (a whole [`crate::decl::VarSpec`]), if one is
    /// recorded.
    pub(crate) fn decode(bits: u32) -> Option<StaticInitFact> {
        let f = (bits & FACT_MASK) >> FACT_SHIFT;
        let init = match f & 7 {
            1 => StaticInit::Constant,
            2 => StaticInit::EarlyStatic,
            3 => StaticInit::Dynamic,
            4 => StaticInit::NotEvaluated(NotEvaluatedWhy::ALL[((f >> 3) & 7) as usize]),
            _ => return None,
        };
        let destruction = match (f >> 6) & 3 {
            1 => Some(StaticDestruction::Trivial),
            2 => Some(StaticDestruction::Constant),
            3 => Some(StaticDestruction::Registered),
            _ => None,
        };
        Some(StaticInitFact { init, destruction })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let inits = [StaticInit::Constant, StaticInit::EarlyStatic, StaticInit::Dynamic]
            .into_iter()
            .chain(NotEvaluatedWhy::ALL.into_iter().map(StaticInit::NotEvaluated));
        for init in inits {
            for destruction in [
                None,
                Some(StaticDestruction::Trivial),
                Some(StaticDestruction::Constant),
                Some(StaticDestruction::Registered),
            ] {
                let f = StaticInitFact { init, destruction };
                let bits = f.encode();
                assert_eq!(bits & !FACT_MASK, 0);
                assert_eq!(StaticInitFact::decode(bits | 0xffff), Some(f));
            }
        }
        assert_eq!(StaticInitFact::decode(0xffff), None);
    }
}
