// [expr.const] clang evaluates the scalar x86 bit builtins
// (`IntExprEvaluator::VisitBuiltinCallExpr`): `lzcnt` / `tzcnt` (the width
// at zero), `bextr` / `bextri` (start and length bytes of the control,
// clamped), `bzhi`, `pdep` and `pext`, whatever the target features (the
// features are a codegen check). spec/oven.md "x86 builtins". WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
static_assert(__builtin_ia32_lzcnt_u32(1) == 31);
static_assert(__builtin_ia32_lzcnt_u16(0) == 16);
static_assert(__builtin_ia32_lzcnt_u64(1ull << 40) == 23);
static_assert(__builtin_ia32_tzcnt_u16(8) == 3);
static_assert(__builtin_ia32_tzcnt_u32(0) == 32);
static_assert(__builtin_ia32_tzcnt_u64(0) == 64);
static_assert(__builtin_ia32_bextr_u32(0xF0u, 4 | (4 << 8)) == 0xF);
static_assert(__builtin_ia32_bextr_u32(0xF0u, 4 | (200 << 8)) == 0xF);
static_assert(__builtin_ia32_bextr_u32(0xF0u, 40 | (4 << 8)) == 0);
static_assert(__builtin_ia32_bextr_u64(~0ull, 60 | (8 << 8)) == 0xF);
static_assert(__builtin_ia32_bextri_u32(0xABCDu, 8 | (4 << 8)) == 0xB);
static_assert(__builtin_ia32_bzhi_si(0xFFu, 4) == 0xF);
static_assert(__builtin_ia32_bzhi_si(0xFFu, 40) == 0xFF);
static_assert(__builtin_ia32_bzhi_di(~0ull, 0x101) == 1);
static_assert(__builtin_ia32_pdep_si(0xFu, 0xF0u) == 0xF0);
static_assert(__builtin_ia32_pdep_si(0x5u, 0xA5u) == 0x21);
static_assert(__builtin_ia32_pdep_di(~0ull, 1ull << 63) == 1ull << 63);
static_assert(__builtin_ia32_pext_si(0xF0u, 0xF0u) == 0xF);
static_assert(__builtin_ia32_pext_di(0x8000000000000001ull, 0x8000000000000001ull) == 3);

constexpr unsigned folded = __builtin_ia32_tzcnt_u32(0x100);
static_assert(folded == 8);
