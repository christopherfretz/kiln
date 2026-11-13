//! Function prototypes (spec/types.md §5.4).

use std::fmt;

use super::kinds::{CallConv, Quals, RefQual};
use crate::ids::{DeclId, ExprId, ProfileId, QualType};

/// The packed per-function bits: `variadic | ref_qual(2) | this_quals(3) |
/// cc(4) | noreturn | trailing_return | no_caller_saved_registers |
/// regparm(3)`
/// (spec/types.md §5.4). Built with the
/// `with_*` methods.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, kiln_snap::Snap)]
pub struct FuncInfo(u16);

impl FuncInfo {
    const VARIADIC: u16 = 1;
    const REF_SHIFT: u16 = 1;
    const QUALS_SHIFT: u16 = 3;
    const CC_SHIFT: u16 = 6;
    const NORETURN: u16 = 1 << 10;
    const TRAILING: u16 = 1 << 11;
    const NO_CSR: u16 = 1 << 12;
    const REGPARM_SHIFT: u16 = 13;

    /// No bits set: a non-variadic, non-member function with the default
    /// calling convention.
    pub const fn new() -> FuncInfo {
        FuncInfo(0)
    }

    /// The raw bits.
    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// The info with raw bits `bits`.
    #[inline]
    pub const fn from_bits(bits: u16) -> FuncInfo {
        FuncInfo(bits)
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
            FuncInfo(self.0 | (3 << Self::REF_SHIFT) | Self::VARIADIC)
        } else if self.no_proto() {
            FuncInfo(self.0 & !(3 << Self::REF_SHIFT))
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
        FuncInfo(if v { self.0 | Self::NO_CSR } else { self.0 & !Self::NO_CSR })
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
            Some(n) if n < 7 => n as u16 + 1,
            _ => 0,
        };
        FuncInfo((self.0 & !(7 << Self::REGPARM_SHIFT)) | (n << Self::REGPARM_SHIFT))
    }

    /// With the variadic bit set to `v`.
    #[must_use]
    pub const fn with_variadic(self, v: bool) -> FuncInfo {
        FuncInfo(if v { self.0 | Self::VARIADIC } else { self.0 & !Self::VARIADIC })
    }

    /// With ref-qualifier `r`.
    #[must_use]
    pub const fn with_ref_qual(self, r: RefQual) -> FuncInfo {
        FuncInfo((self.0 & !(3 << Self::REF_SHIFT)) | ((r as u16) << Self::REF_SHIFT))
    }

    /// With method qualifiers `q`.
    #[must_use]
    pub const fn with_this_quals(self, q: Quals) -> FuncInfo {
        FuncInfo((self.0 & !(7 << Self::QUALS_SHIFT)) | ((q.bits() as u16) << Self::QUALS_SHIFT))
    }

    /// With calling convention `cc`.
    #[must_use]
    pub const fn with_cc(self, cc: CallConv) -> FuncInfo {
        FuncInfo((self.0 & !(15 << Self::CC_SHIFT)) | ((cc as u16) << Self::CC_SHIFT))
    }

    /// With the `noreturn` bit set to `v`.
    #[must_use]
    pub const fn with_noreturn(self, v: bool) -> FuncInfo {
        FuncInfo(if v { self.0 | Self::NORETURN } else { self.0 & !Self::NORETURN })
    }

    /// With the trailing-return bit set to `v`.
    #[must_use]
    pub const fn with_trailing_return(self, v: bool) -> FuncInfo {
        FuncInfo(if v { self.0 | Self::TRAILING } else { self.0 & !Self::TRAILING })
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

/// A function prototype's side record: 16 bytes (spec/types.md §5.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct FuncProto {
    pub(crate) ret: QualType,
    pub(crate) params: u32,
    pub(crate) n_params: u16,
    pub(crate) info: FuncInfo,
    /// 3-bit tag (low bits) + 29-bit operand.
    pub(crate) exc: u32,
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
        self.info
    }

    /// True for `(..., ...)`.
    #[inline]
    pub const fn variadic(&self) -> bool {
        self.info.variadic()
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
