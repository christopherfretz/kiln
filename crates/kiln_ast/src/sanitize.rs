//! Sanitizer names and masks (clang's `Sanitizers.def`), shared by the
//! driver (`-fsanitize=`), sema (`no_sanitize`) and codegen
//! (docs/design/sanitizers.md).
#![warn(missing_docs)]

/// A set of sanitizers: one bit per [`SANITIZERS`] entry.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct SanitizerSet(pub u64);

macro_rules! sanitizers {
    ($($konst:ident = $bit:literal, $name:literal;)*) => {
        #[allow(missing_docs, reason = "each constant is named after the sanitizer it denotes")]
        impl SanitizerSet {
            $(pub const $konst: SanitizerSet = SanitizerSet(1 << $bit);)*
        }
        /// Every single sanitizer: its `-fsanitize=` name and bit.
        pub const SANITIZERS: &[(&str, SanitizerSet)] = &[$(($name, SanitizerSet::$konst)),*];
    };
}

sanitizers! {
    ADDRESS = 0, "address";
    POINTER_COMPARE = 1, "pointer-compare";
    POINTER_SUBTRACT = 2, "pointer-subtract";
    KERNEL_ADDRESS = 3, "kernel-address";
    HWADDRESS = 4, "hwaddress";
    KERNEL_HWADDRESS = 5, "kernel-hwaddress";
    MEMTAG_STACK = 6, "memtag-stack";
    MEMTAG_HEAP = 7, "memtag-heap";
    MEMTAG_GLOBALS = 8, "memtag-globals";
    THREAD = 9, "thread";
    MEMORY = 10, "memory";
    KERNEL_MEMORY = 11, "kernel-memory";
    LEAK = 12, "leak";
    FUZZER = 13, "fuzzer";
    FUZZER_NO_LINK = 14, "fuzzer-no-link";
    DATAFLOW = 15, "dataflow";
    SAFE_STACK = 16, "safe-stack";
    SHADOW_CALL_STACK = 17, "shadow-call-stack";
    SCUDO = 18, "scudo";
    NUMERICAL = 19, "numerical";
    KCFI = 20, "kcfi";
    CFI_CAST_STRICT = 21, "cfi-cast-strict";
    CFI_DERIVED_CAST = 22, "cfi-derived-cast";
    CFI_UNRELATED_CAST = 23, "cfi-unrelated-cast";
    CFI_NVCALL = 24, "cfi-nvcall";
    CFI_VCALL = 25, "cfi-vcall";
    CFI_ICALL = 26, "cfi-icall";
    CFI_MFCALL = 27, "cfi-mfcall";
    ALIGNMENT = 28, "alignment";
    ARRAY_BOUNDS = 29, "array-bounds";
    LOCAL_BOUNDS = 30, "local-bounds";
    BOOL = 31, "bool";
    BUILTIN = 32, "builtin";
    ENUM = 33, "enum";
    FLOAT_CAST_OVERFLOW = 34, "float-cast-overflow";
    FLOAT_DIVIDE_BY_ZERO = 35, "float-divide-by-zero";
    FUNCTION = 36, "function";
    INTEGER_DIVIDE_BY_ZERO = 37, "integer-divide-by-zero";
    NONNULL_ATTRIBUTE = 38, "nonnull-attribute";
    NULL = 39, "null";
    NULLABILITY_ARG = 40, "nullability-arg";
    NULLABILITY_ASSIGN = 41, "nullability-assign";
    NULLABILITY_RETURN = 42, "nullability-return";
    OBJECT_SIZE = 43, "object-size";
    POINTER_OVERFLOW = 44, "pointer-overflow";
    RETURN = 45, "return";
    RETURNS_NONNULL_ATTRIBUTE = 46, "returns-nonnull-attribute";
    SHIFT_BASE = 47, "shift-base";
    SHIFT_EXPONENT = 48, "shift-exponent";
    SIGNED_INTEGER_OVERFLOW = 49, "signed-integer-overflow";
    UNREACHABLE = 50, "unreachable";
    VLA_BOUND = 51, "vla-bound";
    VPTR = 52, "vptr";
    UNSIGNED_INTEGER_OVERFLOW = 53, "unsigned-integer-overflow";
    UNSIGNED_SHIFT_BASE = 54, "unsigned-shift-base";
    IMPLICIT_UNSIGNED_INTEGER_TRUNCATION = 55, "implicit-unsigned-integer-truncation";
    IMPLICIT_SIGNED_INTEGER_TRUNCATION = 56, "implicit-signed-integer-truncation";
    IMPLICIT_INTEGER_SIGN_CHANGE = 57, "implicit-integer-sign-change";
    IMPLICIT_BITFIELD_CONVERSION = 58, "implicit-bitfield-conversion";
    OBJC_CAST = 59, "objc-cast";
}

impl SanitizerSet {
    /// No sanitizer.
    pub const NONE: SanitizerSet = SanitizerSet(0);
    /// Every sanitizer (`all`).
    pub const ALL: SanitizerSet = SanitizerSet((1 << 60) - 1);
    /// `shift`.
    pub const SHIFT: SanitizerSet = SanitizerSet(Self::SHIFT_BASE.0 | Self::SHIFT_EXPONENT.0);
    /// `undefined`.
    pub const UNDEFINED: SanitizerSet = SanitizerSet(
        Self::ALIGNMENT.0
            | Self::BOOL.0
            | Self::BUILTIN.0
            | Self::ARRAY_BOUNDS.0
            | Self::ENUM.0
            | Self::FLOAT_CAST_OVERFLOW.0
            | Self::INTEGER_DIVIDE_BY_ZERO.0
            | Self::NONNULL_ATTRIBUTE.0
            | Self::NULL.0
            | Self::OBJECT_SIZE.0
            | Self::POINTER_OVERFLOW.0
            | Self::RETURN.0
            | Self::RETURNS_NONNULL_ATTRIBUTE.0
            | Self::SHIFT.0
            | Self::SIGNED_INTEGER_OVERFLOW.0
            | Self::UNREACHABLE.0
            | Self::VLA_BOUND.0
            | Self::FUNCTION.0
            | Self::VPTR.0,
    );
    /// `implicit-integer-truncation`.
    pub const IMPLICIT_INTEGER_TRUNCATION: SanitizerSet = SanitizerSet(
        Self::IMPLICIT_UNSIGNED_INTEGER_TRUNCATION.0 | Self::IMPLICIT_SIGNED_INTEGER_TRUNCATION.0,
    );
    /// `implicit-integer-arithmetic-value-change`.
    pub const IMPLICIT_INTEGER_ARITHMETIC_VALUE_CHANGE: SanitizerSet = SanitizerSet(
        Self::IMPLICIT_INTEGER_SIGN_CHANGE.0 | Self::IMPLICIT_SIGNED_INTEGER_TRUNCATION.0,
    );
    /// `implicit-integer-conversion`.
    pub const IMPLICIT_INTEGER_CONVERSION: SanitizerSet = SanitizerSet(
        Self::IMPLICIT_INTEGER_ARITHMETIC_VALUE_CHANGE.0
            | Self::IMPLICIT_UNSIGNED_INTEGER_TRUNCATION.0,
    );
    /// `implicit-conversion`.
    pub const IMPLICIT_CONVERSION: SanitizerSet =
        SanitizerSet(Self::IMPLICIT_INTEGER_CONVERSION.0 | Self::IMPLICIT_BITFIELD_CONVERSION.0);
    /// `integer`.
    pub const INTEGER: SanitizerSet = SanitizerSet(
        Self::IMPLICIT_CONVERSION.0
            | Self::INTEGER_DIVIDE_BY_ZERO.0
            | Self::SHIFT.0
            | Self::SIGNED_INTEGER_OVERFLOW.0
            | Self::UNSIGNED_INTEGER_OVERFLOW.0
            | Self::UNSIGNED_SHIFT_BASE.0,
    );
    /// `nullability`.
    pub const NULLABILITY: SanitizerSet = SanitizerSet(
        Self::NULLABILITY_ARG.0 | Self::NULLABILITY_ASSIGN.0 | Self::NULLABILITY_RETURN.0,
    );
    /// `bounds`.
    pub const BOUNDS: SanitizerSet = SanitizerSet(Self::ARRAY_BOUNDS.0 | Self::LOCAL_BOUNDS.0);
    /// `cfi`.
    pub const CFI: SanitizerSet = SanitizerSet(
        Self::CFI_DERIVED_CAST.0
            | Self::CFI_ICALL.0
            | Self::CFI_MFCALL.0
            | Self::CFI_UNRELATED_CAST.0
            | Self::CFI_NVCALL.0
            | Self::CFI_VCALL.0,
    );
    /// `memtag`.
    pub const MEMTAG: SanitizerSet =
        SanitizerSet(Self::MEMTAG_STACK.0 | Self::MEMTAG_HEAP.0 | Self::MEMTAG_GLOBALS.0);
    /// clang's `TrappingSupported`.
    pub const TRAPPING_SUPPORTED: SanitizerSet = SanitizerSet(
        Self::UNDEFINED.0
            | Self::INTEGER.0
            | Self::NULLABILITY.0
            | Self::LOCAL_BOUNDS.0
            | Self::CFI.0
            | Self::FLOAT_DIVIDE_BY_ZERO.0
            | Self::OBJC_CAST.0,
    );
    /// clang's `Unrecoverable`: always abort.
    pub const UNRECOVERABLE: SanitizerSet = SanitizerSet(Self::UNREACHABLE.0 | Self::RETURN.0);
    /// clang's `RecoverableByDefault`.
    pub const RECOVERABLE_BY_DEFAULT: SanitizerSet = SanitizerSet(
        (Self::UNDEFINED.0
            | Self::INTEGER.0
            | Self::NULLABILITY.0
            | Self::FLOAT_DIVIDE_BY_ZERO.0
            | Self::OBJC_CAST.0)
            & !Self::UNRECOVERABLE.0,
    );
    /// The sanitizers `-fsanitize-recover=` may name (clang's
    /// `Recoverable`, plus the pass sanitizers it accepts).
    pub const RECOVERABLE: SanitizerSet = SanitizerSet(
        Self::RECOVERABLE_BY_DEFAULT.0
            | Self::ADDRESS.0
            | Self::HWADDRESS.0
            | Self::KERNEL_ADDRESS.0
            | Self::KERNEL_HWADDRESS.0
            | Self::MEMORY.0
            | Self::KERNEL_MEMORY.0
            | Self::THREAD.0
            | Self::CFI.0
            | Self::LOCAL_BOUNDS.0,
    );
    /// The sanitizers kiln implements or accepts (WP-675): the pass
    /// sanitizers whose passes it schedules, the `undefined` family
    /// (spec/codegen.md lists the checks not emitted yet) and
    /// unsigned-integer-overflow.
    pub const KILN_SUPPORTED: SanitizerSet = SanitizerSet(
        Self::ADDRESS.0
            | Self::HWADDRESS.0
            | Self::THREAD.0
            | Self::MEMORY.0
            | Self::LEAK.0
            | Self::FUZZER.0
            | Self::FUZZER_NO_LINK.0
            | Self::UNDEFINED.0
            | Self::UNSIGNED_INTEGER_OVERFLOW.0,
    );

    /// Whether any bit of `o` is set.
    #[must_use]
    pub const fn has_any(self, o: SanitizerSet) -> bool {
        self.0 & o.0 != 0
    }
    /// Whether every bit of `o` is set.
    #[must_use]
    pub const fn has(self, o: SanitizerSet) -> bool {
        self.0 & o.0 == o.0 && o.0 != 0
    }
    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// The union.
    #[must_use]
    pub const fn union(self, o: SanitizerSet) -> SanitizerSet {
        SanitizerSet(self.0 | o.0)
    }
    /// The intersection.
    #[must_use]
    pub const fn intersect(self, o: SanitizerSet) -> SanitizerSet {
        SanitizerSet(self.0 & o.0)
    }
    /// The difference.
    #[must_use]
    pub const fn without(self, o: SanitizerSet) -> SanitizerSet {
        SanitizerSet(self.0 & !o.0)
    }
    /// The low and high words, for an `AttrNode`'s `a` and `b`.
    #[must_use]
    pub const fn words(self) -> (u32, u32) {
        #[allow(clippy::cast_possible_truncation, reason = "split into two words")]
        (self.0 as u32, (self.0 >> 32) as u32)
    }
    /// The set from [`Self::words`].
    #[must_use]
    pub const fn from_words(a: u32, b: u32) -> SanitizerSet {
        SanitizerSet((a as u64) | ((b as u64) << 32))
    }
    /// The names of the single sanitizers in the set, in table order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        SANITIZERS.iter().filter(move |(_, s)| self.has(*s)).map(|(n, _)| *n)
    }
}

/// The groups clang accepts by name, with their members.
pub const GROUPS: &[(&str, SanitizerSet)] = &[
    ("all", SanitizerSet::ALL),
    ("bounds", SanitizerSet::BOUNDS),
    ("cfi", SanitizerSet::CFI),
    ("implicit-conversion", SanitizerSet::IMPLICIT_CONVERSION),
    (
        "implicit-integer-arithmetic-value-change",
        SanitizerSet::IMPLICIT_INTEGER_ARITHMETIC_VALUE_CHANGE,
    ),
    ("implicit-integer-conversion", SanitizerSet::IMPLICIT_INTEGER_CONVERSION),
    ("implicit-integer-truncation", SanitizerSet::IMPLICIT_INTEGER_TRUNCATION),
    ("integer", SanitizerSet::INTEGER),
    ("memtag", SanitizerSet::MEMTAG),
    ("nullability", SanitizerSet::NULLABILITY),
    ("shift", SanitizerSet::SHIFT),
    ("undefined", SanitizerSet::UNDEFINED),
];

/// The set a sanitizer or group name denotes (clang's
/// `parseSanitizerValue`), `None` for an unknown name. `all` is only
/// meaningful where `allow_all` (the `-fno-`, recover and trap lists;
/// clang reports it as unsupported in `-fsanitize=`).
#[must_use]
pub fn parse_name(name: &str, allow_all: bool) -> Option<SanitizerSet> {
    if name == "all" && !allow_all {
        return None;
    }
    SANITIZERS.iter().chain(GROUPS).find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The `no_sanitize*` attribute spellings (`AttrKind::NO_SANITIZE`) and
/// the set each one names (`no_sanitize("...")` names its arguments'). An
/// attribute node keeps the set in `a` and `b` ([`SanitizerSet::words`]),
/// and its spelling's index here in `b`'s top four bits (the set has no
/// bit above 59).
pub const NO_SANITIZE_SPELLINGS: [(&str, SanitizerSet); 5] = [
    ("no_sanitize", SanitizerSet::NONE),
    ("no_sanitize_address", SanitizerSet::ADDRESS),
    ("no_address_safety_analysis", SanitizerSet::ADDRESS),
    ("no_sanitize_memory", SanitizerSet::MEMORY),
    ("no_sanitize_thread", SanitizerSet::THREAD),
];

/// The `a` and `b` words of a `no_sanitize*` attribute node of spelling
/// `tag` ([`NO_SANITIZE_SPELLINGS`]) naming `set`.
#[must_use]
pub const fn no_sanitize_words(set: SanitizerSet, tag: u32) -> (u32, u32) {
    let (a, b) = set.words();
    (a, b | (tag << 28))
}

/// The set a `no_sanitize*` attribute node's words name.
#[must_use]
pub const fn no_sanitize_set(a: u32, b: u32) -> SanitizerSet {
    SanitizerSet::from_words(a, b & 0x0fff_ffff)
}

/// The spelling index ([`NO_SANITIZE_SPELLINGS`]) of a `no_sanitize*`
/// attribute node's `b` word.
#[must_use]
pub const fn no_sanitize_tag(b: u32) -> usize {
    (b >> 28) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_groups() {
        assert_eq!(parse_name("address", false), Some(SanitizerSet::ADDRESS));
        assert_eq!(parse_name("shift", false), Some(SanitizerSet::SHIFT));
        assert_eq!(parse_name("all", false), None);
        assert_eq!(parse_name("all", true), Some(SanitizerSet::ALL));
        assert_eq!(parse_name("foo", true), None);
        let u: Vec<_> = SanitizerSet::UNDEFINED.names().collect();
        assert_eq!(u.len(), 20);
        assert!(!SanitizerSet::RECOVERABLE_BY_DEFAULT.has_any(SanitizerSet::RETURN));
        let (a, b) = SanitizerSet::UNDEFINED.words();
        assert_eq!(SanitizerSet::from_words(a, b), SanitizerSet::UNDEFINED);
        // One bit per entry, all distinct.
        let mut all = 0u64;
        for (_, s) in SANITIZERS {
            assert_eq!(s.0.count_ones(), 1);
            assert_eq!(all & s.0, 0);
            all |= s.0;
        }
        assert_eq!(all, SanitizerSet::ALL.0);
    }
}
