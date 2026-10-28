//! Target descriptions (spec/abi.md §2): the facts about a target that
//! the frontend needs before LLVM sees the module, for the two M2
//! targets `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
//!
//! Every number here is clang 19's (`clang/lib/Basic/Targets/X86.h`,
//! `AArch64.cpp`) and is checked against `clang++-19` by the unit tests
//! and by `tests/layout_vs_clang.rs`.

use std::fmt;

use kiln_ast::TypeId;
use kiln_ast::types::{BuiltinType, FloatFormat, TargetTypes};

/// A CPU architecture kiln can target.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Arch {
    /// x86-64 (System V psABI).
    X86_64,
    /// `AArch64` (AAPCS64).
    AArch64,
}

impl Arch {
    /// The canonical triple spelling of the architecture.
    pub const fn name(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::AArch64 => "aarch64",
        }
    }
}

/// A parsed target triple. Only `<arch>[-<vendor>]-linux-gnu` is
/// supported in M2.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Triple {
    /// The architecture.
    pub arch: Arch,
}

/// An unsupported or malformed target triple.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TripleError {
    /// The triple as given.
    pub triple: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

impl fmt::Display for TripleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported target triple '{}': {}", self.triple, self.reason)
    }
}

impl std::error::Error for TripleError {}

impl Triple {
    /// Parses a triple as clang's driver accepts it for the M2 targets:
    /// `x86_64-unknown-linux-gnu`, `x86_64-linux-gnu`,
    /// `x86_64-pc-linux-gnu`, `amd64-…`, `aarch64-unknown-linux-gnu`,
    /// `aarch64-linux-gnu`, `arm64-…`. Components are case-sensitive, as
    /// in LLVM's `Triple`.
    pub fn parse(s: &str) -> Result<Triple, TripleError> {
        let err = |reason| Err(TripleError { triple: s.to_owned(), reason });
        let parts: Vec<&str> = s.split('-').collect();
        let arch = match parts[0] {
            "x86_64" | "amd64" | "x86_64h" => Arch::X86_64,
            "aarch64" | "arm64" => Arch::AArch64,
            "" => return err("empty architecture"),
            _ => return err("unsupported architecture (M2 supports x86_64 and aarch64)"),
        };
        let (vendor, os, env) = match parts.len() {
            3 => ("unknown", parts[1], parts[2]),
            4 => (parts[1], parts[2], parts[3]),
            _ => return err("expected <arch>[-<vendor>]-linux-gnu"),
        };
        if !matches!(vendor, "unknown" | "pc" | "none" | "") {
            return err("unsupported vendor");
        }
        if os != "linux" {
            return err("unsupported operating system (M2 supports linux)");
        }
        if env != "gnu" {
            return err("unsupported environment (M2 supports gnu)");
        }
        Ok(Triple { arch })
    }

    /// The normalized spelling, `<arch>-unknown-linux-gnu`.
    pub fn normalized(self) -> String {
        format!("{}-unknown-linux-gnu", self.arch.name())
    }
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-unknown-linux-gnu", self.arch.name())
    }
}

/// Everything the frontend knows about a target (spec/abi.md §2).
///
/// Sizes and alignments are in bytes unless the field name says bits.
#[derive(Clone, PartialEq, Eq, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent target facts, as clang's TargetInfo")]
pub struct Target {
    /// The triple.
    pub triple: Triple,
    /// The builtin-type facts stored in `TypeContext` (spec/types.md §11).
    pub types: TargetTypes,
    /// Size and alignment of a data or function pointer.
    pub pointer_size: u64,
    /// Size of a pointer to data member (Itanium: one `ptrdiff_t`).
    pub data_member_pointer_size: u64,
    /// Size of a pointer to member function (Itanium: `{ptr, adj}`).
    pub function_member_pointer_size: u64,
    /// `sizeof(std::max_align_t)`.
    pub max_align_t_size: u64,
    /// `alignof(std::max_align_t)`.
    pub max_align_t_align: u64,
    /// `__STDCPP_DEFAULT_NEW_ALIGNMENT__`.
    pub default_new_align: u64,
    /// `__BIGGEST_ALIGNMENT__`: the alignment `__attribute__((aligned))`
    /// with no argument requests.
    pub biggest_align: u64,
    /// The target is big-endian (false for both M2 targets).
    pub big_endian: bool,
    /// Unnamed bit-fields (including zero-width ones) affect the record's
    /// alignment (clang `useZeroLengthBitfieldAlignment`: AAPCS64 yes,
    /// x86-64 no).
    pub unnamed_bitfield_affects_align: bool,
    /// Upper clamp, in bits, of `_BitInt(N)` alignment (clang
    /// `getBitIntMaxAlign`).
    pub bit_int_max_align_bits: u64,
    /// The widest `_BitInt` the target supports.
    pub bit_int_max_width: u32,
    /// Widest type, in bits, that `_Atomic` rounds up to a power-of-two
    /// size and alignment (clang `MaxAtomicPromoteWidth`).
    pub max_atomic_promote_bits: u64,
    /// Cap on vector alignment in bits (0 = none; clang `MaxVectorAlign`).
    pub max_vector_align_bits: u64,
    /// `__float128` is available.
    pub has_float128: bool,
    /// `__fp16` is available (storage only).
    pub has_fp16: bool,
}

impl Target {
    /// The description of `arch`-unknown-linux-gnu.
    pub fn for_arch(arch: Arch) -> Target {
        let x86 = arch == Arch::X86_64;
        Target {
            triple: Triple { arch },
            types: target_types(arch),
            pointer_size: 8,
            data_member_pointer_size: 8,
            function_member_pointer_size: 16,
            max_align_t_size: 32,
            max_align_t_align: 16,
            default_new_align: 16,
            biggest_align: 16,
            big_endian: false,
            unnamed_bitfield_affects_align: !x86,
            bit_int_max_align_bits: if x86 { 64 } else { 128 },
            bit_int_max_width: if x86 { 8_388_608 } else { 128 },
            max_atomic_promote_bits: 128,
            max_vector_align_bits: if x86 { 0 } else { 128 },
            has_float128: x86,
            has_fp16: true,
        }
    }

    /// Parses `triple` ([`Triple::parse`]) and describes it.
    pub fn from_triple(triple: &str) -> Result<Target, TripleError> {
        Triple::parse(triple).map(|t| Target::for_arch(t.arch))
    }

    /// The architecture.
    pub const fn arch(&self) -> Arch {
        self.triple.arch
    }

    /// `(size, align)` in bytes of builtin `b` (`(0, 1)` for `void`, and
    /// `(0, 0)` for the placeholder builtins, as in [`TargetTypes`]).
    pub fn builtin_size_align(&self, b: BuiltinType) -> (u64, u64) {
        let (s, a) = self.types.builtin_size_align(b);
        (u64::from(s), u64::from(a))
    }

    /// `(size, align)` in bits of `_BitInt(width)`: the alignment is
    /// `bit_ceil(clamp(width, 8, max_align))` and the size is the width
    /// rounded up to it (clang `getBitIntAlign`/`getBitIntWidth`).
    pub fn bit_int_size_align_bits(&self, width: u32) -> (u64, u64) {
        let w = u64::from(width.max(1));
        let align = w.clamp(8, self.bit_int_max_align_bits).next_power_of_two();
        (w.div_ceil(align).saturating_mul(align), align)
    }
}

/// The builtin facts of `arch`-unknown-linux-gnu.
fn target_types(arch: Arch) -> TargetTypes {
    use BuiltinType as B;
    let x86 = arch == Arch::X86_64;
    let mut size_align = [(0u16, 0u16); 34];
    let table: [(B, u16, u16); 28] = [
        (B::Void, 0, 1),
        (B::Bool, 1, 1),
        (B::Char, 1, 1),
        (B::SChar, 1, 1),
        (B::UChar, 1, 1),
        (B::WChar, 4, 4),
        (B::Char8, 1, 1),
        (B::Char16, 2, 2),
        (B::Char32, 4, 4),
        (B::Short, 2, 2),
        (B::UShort, 2, 2),
        (B::Int, 4, 4),
        (B::UInt, 4, 4),
        (B::Long, 8, 8),
        (B::ULong, 8, 8),
        (B::LongLong, 8, 8),
        (B::ULongLong, 8, 8),
        (B::Int128, 16, 16),
        (B::UInt128, 16, 16),
        (B::Float16, 2, 2),
        (B::BFloat16, 2, 2),
        (B::Float, 4, 4),
        (B::Double, 8, 8),
        (B::LongDouble, 16, 16),
        (B::Float128, 16, 16),
        (B::Half, 2, 2),
        (B::NullPtr, 8, 8),
        (B::MetaInfo, 8, 8),
    ];
    for (b, s, a) in table {
        size_align[b as usize] = (s, a);
    }
    TargetTypes {
        size_align,
        char_signed: x86,
        wchar_signed: x86,
        long_double: if x86 { FloatFormat::X87Extended } else { FloatFormat::IeeeQuad },
        size_t: TypeId::ULONG,
        ptrdiff_t: TypeId::LONG,
        intptr_t: TypeId::LONG,
        pointer_bytes: 8,
    }
}

/// The target described by `triple` (spec/types.md §11's
/// `kiln_abi::target(triple)`).
pub fn target(triple: &str) -> Result<Target, TripleError> {
    Target::from_triple(triple)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triple_spellings_and_aliases() {
        for s in [
            "x86_64-unknown-linux-gnu",
            "x86_64-linux-gnu",
            "x86_64-pc-linux-gnu",
            "amd64-linux-gnu",
        ] {
            assert_eq!(Triple::parse(s).unwrap().arch, Arch::X86_64, "{s}");
        }
        for s in [
            "aarch64-unknown-linux-gnu",
            "aarch64-linux-gnu",
            "arm64-linux-gnu",
            "arm64-unknown-linux-gnu",
        ] {
            assert_eq!(Triple::parse(s).unwrap().arch, Arch::AArch64, "{s}");
        }
        for s in [
            "",
            "riscv64-linux-gnu",
            "x86_64-apple-darwin",
            "x86_64-linux",
            "aarch64-linux-musl",
            "x86_64-w64-windows-gnu",
        ] {
            assert!(Triple::parse(s).is_err(), "{s}");
        }
        assert_eq!(
            Triple::parse("arm64-linux-gnu").unwrap().to_string(),
            "aarch64-unknown-linux-gnu"
        );
        assert_eq!(
            target("amd64-pc-linux-gnu").unwrap().triple.normalized(),
            "x86_64-unknown-linux-gnu"
        );
    }

    /// [basic.fundamental]: sizes are implementation-defined; these are
    /// clang 19's for the two psABIs.
    #[test]
    fn builtin_sizes() {
        for arch in [Arch::X86_64, Arch::AArch64] {
            let t = Target::for_arch(arch);
            assert_eq!(t.builtin_size_align(BuiltinType::LongDouble), (16, 16));
            assert_eq!(t.builtin_size_align(BuiltinType::Int128), (16, 16));
            assert_eq!(t.builtin_size_align(BuiltinType::WChar), (4, 4));
            assert_eq!(t.builtin_size_align(BuiltinType::Char8), (1, 1));
            assert_eq!(t.builtin_size_align(BuiltinType::Float16), (2, 2));
            assert_eq!(t.builtin_size_align(BuiltinType::BFloat16), (2, 2));
            assert_eq!(t.builtin_size_align(BuiltinType::NullPtr), (8, 8));
            assert_eq!(t.builtin_size_align(BuiltinType::Long), (8, 8));
            assert_eq!(t.types.pointer_bytes, 8);
            assert_eq!(t.types.size_t, TypeId::ULONG);
            assert!(!t.big_endian);
            assert_eq!((t.max_align_t_size, t.max_align_t_align), (32, 16));
        }
        let x = Target::for_arch(Arch::X86_64);
        let a = Target::for_arch(Arch::AArch64);
        assert!(x.types.char_signed && x.types.wchar_signed);
        assert!(!a.types.char_signed && !a.types.wchar_signed);
        assert_eq!(x.types.long_double, FloatFormat::X87Extended);
        assert_eq!(a.types.long_double, FloatFormat::IeeeQuad);
    }

    /// `_BitInt(N)` ([basic.fundamental]/1 bit-precise integers).
    #[test]
    fn bit_int_layout() {
        let x = Target::for_arch(Arch::X86_64);
        let a = Target::for_arch(Arch::AArch64);
        assert_eq!(x.bit_int_size_align_bits(17), (32, 32));
        assert_eq!(x.bit_int_size_align_bits(9), (16, 16));
        assert_eq!(x.bit_int_size_align_bits(33), (64, 64));
        assert_eq!(x.bit_int_size_align_bits(1), (8, 8));
        assert_eq!(x.bit_int_size_align_bits(65), (128, 64));
        assert_eq!(a.bit_int_size_align_bits(65), (128, 128));
        assert_eq!(x.bit_int_size_align_bits(129), (192, 64));
    }
}
