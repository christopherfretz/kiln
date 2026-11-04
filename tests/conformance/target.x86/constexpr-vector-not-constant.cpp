// [expr.const]/10: clang 19 and 22 evaluate no x86 vector builtin; a call
// in a constant expression is not one (spec/oven.md "x86 builtins"),
// which `-fsyntax-only` reports at the call. WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:45 note "subexpression not valid in a constant expression"
//@ diag: 12:15 note "in call to 'mask()'"
typedef char v16qi __attribute__((vector_size(16)));
constexpr int mask() { v16qi v = {}; return __builtin_ia32_pmovmskb128(v); }
static_assert(mask() == 0);
