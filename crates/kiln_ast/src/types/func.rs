//! Function prototypes (spec/types.md §5.4).

use std::fmt;

use super::kinds::{CallConv, Quals, RefQual};
use crate::ids::{DeclId, ExprId, ProfileId, QualType};

/// The `AArch64` SME function type attributes (ACLE; clang
/// `FunctionType::AArch64SMETypeAttributes`), packed as clang's Itanium
/// mangling spells them (`AAPCSBitmaskSME`): [`sme::STREAMING`],
/// [`sme::STREAMING_COMPATIBLE`], [`sme::AGNOSTIC_ZA`], then the ZA and ZT0
/// states (3 bits each: [`sme::IN`], [`sme::OUT`], [`sme::INOUT`],
/// [`sme::PRESERVES`]) at [`sme::ZA_SHIFT`] and [`sme::ZT0_SHIFT`].
pub mod sme {
    /// `__arm_streaming`.
    pub const STREAMING: u16 = 1;
    /// `__arm_streaming_compatible`.
    pub const STREAMING_COMPATIBLE: u16 = 2;
    /// `__arm_agnostic("sme_za_state")`.
    pub const AGNOSTIC_ZA: u16 = 4;
    /// The ZA state's position.
    pub const ZA_SHIFT: u16 = 3;
    /// The ZT0 state's position.
    pub const ZT0_SHIFT: u16 = 6;
    /// `__arm_in`.
    pub const IN: u16 = 1;
    /// `__arm_out`.
    pub const OUT: u16 = 2;
    /// `__arm_inout`.
    pub const INOUT: u16 = 3;
    /// `__arm_preserves`.
    pub const PRESERVES: u16 = 4;
    /// Every bit in use.
    pub const MASK: u16 = 0x1ff;

    /// The ZA state of `bits` (0: none).
    pub const fn za(bits: u16) -> u16 {
        (bits >> ZA_SHIFT) & 7
    }

    /// The ZT0 state of `bits` (0: none).
    pub const fn zt0(bits: u16) -> u16 {
        (bits >> ZT0_SHIFT) & 7
    }
}

/// The packed per-function bits: `variadic | ref_qual(2) | this_quals(3) |
/// cc(4) | noreturn | trailing_return | no_caller_saved_registers |
/// regparm(3) | sme(9)`
/// (spec/types.md §5.4). Built with the
/// `with_*` methods.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
pub struct FuncInfo(u32, u16);

impl FuncInfo {
    const VARIADIC: u32 = 1;
    const REF_SHIFT: u32 = 1;
    const QUALS_SHIFT: u32 = 3;
    const CC_SHIFT: u32 = 6;
    const NORETURN: u32 = 1 << 10;
    const TRAILING: u32 = 1 << 11;
    const NO_CSR: u32 = 1 << 12;
    const REGPARM_SHIFT: u32 = 13;
    const SME_SHIFT: u32 = 16;

    /// No bits set: a non-variadic, non-member function with the default
    /// calling convention.
    pub const fn new() -> FuncInfo {
        FuncInfo(0, 0)
    }

    /// The raw bits.
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// The info with raw bits `bits` (the unused high bits cleared) and
    /// no parameter ABIs.
    #[inline]
    pub const fn from_bits(bits: u32) -> FuncInfo {
        FuncInfo(bits & ((1 << (Self::SME_SHIFT + 9)) - 1), 0)
    }

    /// `bits` with this info's parameter ABIs.
    #[inline]
    const fn raw(self, bits: u32) -> FuncInfo {
        FuncInfo(bits, self.1)
    }

    /// The id of the parameter ABI list in the type context (clang
    /// `FunctionProtoType::ExtParameterInfo`'s ABI; 0: every parameter
    /// is ordinary; [`ParamAbi`], `TypeContext::param_abi`; WP-1745).
    #[inline]
    pub const fn param_abis(self) -> u16 {
        self.1
    }

    /// With parameter ABI list `id` (made by
    /// `TypeContext::intern_param_abis`).
    #[must_use]
    pub const fn with_param_abis(self, id: u16) -> FuncInfo {
        FuncInfo(self.0, id)
    }

    /// The `AArch64` SME attributes ([`sme`]; 0: none).
    #[inline]
    pub const fn sme(self) -> u16 {
        ((self.0 >> Self::SME_SHIFT) as u16) & sme::MASK
    }

    /// With SME attributes `bits` ([`sme`]).
    #[must_use]
    pub const fn with_sme(self, bits: u16) -> FuncInfo {
        self.raw(
            (self.0 & !((sme::MASK as u32) << Self::SME_SHIFT))
                | (((bits & sme::MASK) as u32) << Self::SME_SHIFT),
        )
    }

    /// True for `(..., ...)`.
    #[inline]
    pub const fn variadic(self) -> bool {
        self.0 & Self::VARIADIC != 0
    }

    /// True for a C function type without a prototype (C17 6.7.6.3p14:
    /// `int f()` before C23; clang `FunctionNoProtoType`). Encoded as the
    /// otherwise unused ref-qualifier value 3, so it never coexists with a
    /// ref-qualifier; such a type is also variadic with no parameters, so
    /// a call applies the default argument promotions and the IR type is
    /// `ret (...)`.
    #[inline]
    pub const fn no_proto(self) -> bool {
        (self.0 >> Self::REF_SHIFT) & 3 == 3
    }

    /// With the no-prototype marker set (and the variadic bit, see
    /// [`FuncInfo::no_proto`]) or cleared.
    #[must_use]
    pub const fn with_no_proto(self, v: bool) -> FuncInfo {
        if v {
            self.raw(self.0 | (3 << Self::REF_SHIFT) | Self::VARIADIC)
        } else if self.no_proto() {
            self.raw(self.0 & !(3 << Self::REF_SHIFT))
        } else {
            self
        }
    }

    /// The method's ref-qualifier.
    #[inline]
    pub const fn ref_qual(self) -> RefQual {
        match (self.0 >> Self::REF_SHIFT) & 3 {
            1 => RefQual::LValue,
            2 => RefQual::RValue,
            _ => RefQual::None,
        }
    }

    /// The method's cv-qualifiers (the implicit object's).
    #[inline]
    pub const fn this_quals(self) -> Quals {
        Quals::from_bits(((self.0 >> Self::QUALS_SHIFT) & 7) as u8)
    }

    /// The calling convention.
    #[inline]
    pub fn cc(self) -> CallConv {
        CallConv::from_u8(((self.0 >> Self::CC_SHIFT) & 15) as u8)
    }

    /// True for a `noreturn` function type.
    #[inline]
    pub const fn noreturn(self) -> bool {
        self.0 & Self::NORETURN != 0
    }

    /// True if the return type was written as a trailing return type
    /// (sugar: cleared in the canonical type).
    #[inline]
    pub const fn trailing_return(self) -> bool {
        self.0 & Self::TRAILING != 0
    }

    /// True for an x86 `no_caller_saved_registers` function type (clang
    /// `FunctionType::ExtInfo::getNoCallerSavedRegs`; WP-808).
    #[inline]
    pub const fn no_caller_saved_regs(self) -> bool {
        self.0 & Self::NO_CSR != 0
    }

    /// With the `no_caller_saved_registers` bit set to `v`.
    #[must_use]
    pub const fn with_no_caller_saved_regs(self, v: bool) -> FuncInfo {
        self.raw(if v { self.0 | Self::NO_CSR } else { self.0 & !Self::NO_CSR })
    }

    /// The GNU `regparm(N)` of an x86 function type (clang
    /// `FunctionType::ExtInfo::getHasRegParm` / `getRegParm`; N is 0..=6,
    /// WP-808): None when not written.
    #[inline]
    pub const fn regparm(self) -> Option<u8> {
        match (self.0 >> Self::REGPARM_SHIFT) & 7 {
            0 => None,
            #[allow(clippy::cast_possible_truncation, reason = "masked to 3 bits")]
            n => Some(n as u8 - 1),
        }
    }

    /// With `regparm` set to `v` (N at most 6).
    #[must_use]
    pub const fn with_regparm(self, v: Option<u8>) -> FuncInfo {
        let n = match v {
            Some(n) if n < 7 => n as u32 + 1,
            _ => 0,
        };
        self.raw((self.0 & !(7 << Self::REGPARM_SHIFT)) | (n << Self::REGPARM_SHIFT))
    }

    /// With the variadic bit set to `v`.
    #[must_use]
    pub const fn with_variadic(self, v: bool) -> FuncInfo {
        self.raw(if v { self.0 | Self::VARIADIC } else { self.0 & !Self::VARIADIC })
    }

    /// With ref-qualifier `r`.
    #[must_use]
    pub const fn with_ref_qual(self, r: RefQual) -> FuncInfo {
        self.raw((self.0 & !(3 << Self::REF_SHIFT)) | ((r as u32) << Self::REF_SHIFT))
    }

    /// With method qualifiers `q`.
    #[must_use]
    pub const fn with_this_quals(self, q: Quals) -> FuncInfo {
        self.raw((self.0 & !(7 << Self::QUALS_SHIFT)) | ((q.bits() as u32) << Self::QUALS_SHIFT))
    }

    /// With calling convention `cc`.
    #[must_use]
    pub const fn with_cc(self, cc: CallConv) -> FuncInfo {
        self.raw((self.0 & !(15 << Self::CC_SHIFT)) | ((cc as u32) << Self::CC_SHIFT))
    }

    /// With the `noreturn` bit set to `v`.
    #[must_use]
    pub const fn with_noreturn(self, v: bool) -> FuncInfo {
        self.raw(if v { self.0 | Self::NORETURN } else { self.0 & !Self::NORETURN })
    }

    /// With the trailing-return bit set to `v`.
    #[must_use]
    pub const fn with_trailing_return(self, v: bool) -> FuncInfo {
        self.raw(if v { self.0 | Self::TRAILING } else { self.0 & !Self::TRAILING })
    }
}

impl fmt::Debug for FuncInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FuncInfo")
            .field("variadic", &self.variadic())
            .field("ref_qual", &self.ref_qual())
            .field("this_quals", &self.this_quals())
            .field("cc", &self.cc())
            .field("noreturn", &self.noreturn())
            .field("trailing_return", &self.trailing_return())
            .field("no_caller_saved_regs", &self.no_caller_saved_regs())
            .field("regparm", &self.regparm())
            .field("sme", &self.sme())
            .field("param_abis", &self.param_abis())
            .finish()
    }
}

/// A list of types in the context's `type_lists` arena (dynamic exception
/// specifications), made by `TypeContext::type_list`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct TypeList {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl TypeList {
    /// The empty list.
    pub const EMPTY: TypeList = TypeList { start: 0, len: 0 };

    /// Number of types.
    #[inline]
    pub const fn len(self) -> u32 {
        self.len
    }

    /// True if the list is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// The evaluated value of a `noexcept(expr)` operand.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub enum NoexceptValue {
    /// `noexcept(false)`: potentially throwing.
    False,
    /// `noexcept(true)`: non-throwing.
    True,
    /// Value-dependent; the profile of the expression (spec/types.md §7.2).
    Dependent(ProfileId),
}

/// An exception specification (spec/types.md §5.4), decoded.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, kiln_snap::Snap)]
pub enum ExcSpec {
    /// Potentially throwing (no specification; the canonical form of
    /// `noexcept(false)` and of pre-C++17 specifications).
    #[default]
    None,
    /// Canonical non-throwing.
    Nothrow,
    /// `throw()` as written.
    DynamicNone,
    /// GNU `__attribute__((nothrow))` on a function type with no written
    /// specification (WP-186; clang `EST_NoThrow`): non-throwing.
    NothrowAttr,
    /// `noexcept` as written.
    BasicNoexcept,
    /// `noexcept(expr)` as written.
    NoexceptExpr {
        /// The operand.
        expr: ExprId,
        /// Its value, or its profile when value-dependent.
        value: NoexceptValue,
    },
    /// `throw(A, B)` (C++98..14).
    Dynamic(TypeList),
    /// The canonical form of a value-dependent `noexcept(expr)`.
    DependentNoexcept(ProfileId),
    /// Not yet computed: implicit/defaulted members (unevaluated) or
    /// uninstantiated specializations, told apart by the declaration.
    Deferred(DeclId),
}

impl ExcSpec {
    pub(crate) const TAG_NONE: u32 = 0;
    pub(crate) const TAG_NOTHROW: u32 = 1;
    pub(crate) const TAG_DYNAMIC_NONE: u32 = 2;
    pub(crate) const TAG_BASIC_NOEXCEPT: u32 = 3;
    pub(crate) const TAG_NOEXCEPT_EXPR: u32 = 4;
    pub(crate) const TAG_DYNAMIC: u32 = 5;
    pub(crate) const TAG_DEPENDENT: u32 = 6;
    pub(crate) const TAG_DEFERRED: u32 = 7;

    /// True for the specifications that make a function non-throwing in
    /// its canonical form (from C++17).
    pub fn is_nothrow(self) -> bool {
        matches!(
            self,
            ExcSpec::Nothrow
                | ExcSpec::NothrowAttr
                | ExcSpec::DynamicNone
                | ExcSpec::BasicNoexcept
                | ExcSpec::NoexceptExpr { value: NoexceptValue::True, .. }
        )
    }
}

/// A function prototype's side record: 20 bytes (spec/types.md §5.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct FuncProto {
    pub(crate) ret: QualType,
    pub(crate) params: u32,
    /// [`FuncInfo`]'s bits.
    pub(crate) info: u32,
    /// 3-bit tag (low bits) + 29-bit operand.
    pub(crate) exc: u32,
    pub(crate) n_params: u16,
    /// [`FuncInfo::param_abis`] (in the padding after `n_params`).
    pub(crate) abis: u16,
}

/// A parameter's ABI (clang `ParameterABI`, the Swift parameter
/// attributes; spec/types.md §5.4, WP-1745): part of the function type,
/// as clang's `ExtParameterInfo`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum ParamAbi {
    /// An ordinary parameter.
    #[default]
    Ordinary = 0,
    /// `swift_context` (`swiftself`).
    SwiftContext = 1,
    /// `swift_async_context` (`swiftasync`).
    SwiftAsyncContext = 2,
    /// `swift_error_result` (`swifterror`).
    SwiftErrorResult = 3,
    /// `swift_indirect_result` (`noalias`, `sret` when the result is
    /// `void`).
    SwiftIndirectResult = 4,
}

impl ParamAbi {
    /// The ABI of raw value `v` (an unknown value is ordinary).
    pub const fn from_u8(v: u8) -> ParamAbi {
        match v {
            1 => ParamAbi::SwiftContext,
            2 => ParamAbi::SwiftAsyncContext,
            3 => ParamAbi::SwiftErrorResult,
            4 => ParamAbi::SwiftIndirectResult,
            _ => ParamAbi::Ordinary,
        }
    }

    /// The attribute's name (clang `getParameterABISpelling`; empty for
    /// an ordinary parameter).
    pub const fn spelling(self) -> &'static str {
        match self {
            ParamAbi::Ordinary => "",
            ParamAbi::SwiftContext => "swift_context",
            ParamAbi::SwiftAsyncContext => "swift_async_context",
            ParamAbi::SwiftErrorResult => "swift_error_result",
            ParamAbi::SwiftIndirectResult => "swift_indirect_result",
        }
    }
}

impl FuncProto {
    /// The return type.
    #[inline]
    pub const fn ret(&self) -> QualType {
        self.ret
    }

    /// Number of parameters.
    #[inline]
    pub const fn num_params(&self) -> u32 {
        self.n_params as u32
    }

    /// The packed function bits.
    #[inline]
    pub const fn info(&self) -> FuncInfo {
        FuncInfo(self.info, self.abis)
    }

    /// True for `(..., ...)`.
    #[inline]
    pub const fn variadic(&self) -> bool {
        self.info().variadic()
    }

    #[inline]
    pub(crate) const fn exc_tag(&self) -> u32 {
        self.exc & 7
    }

    #[inline]
    pub(crate) const fn exc_operand(&self) -> u32 {
        self.exc >> 3
    }
}
