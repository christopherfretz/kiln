// [expr.const]/10: clang 19 evaluates no x86 vector builtin, and kiln
// follows it: a call in a constant expression is not one (spec/oven.md
// "x86 builtins"), which `-fsyntax-only` reports at the call. clang 22
// folds `__builtin_ia32_pmovmskb128` (BACKLOG WP-635). WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ skip(clang): clang 22 folds __builtin_ia32_pmovmskb128 in a constant expression
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:45 note "subexpression not valid in a constant expression"
//@ diag: 14:15 note "in call to 'mask()'"
typedef char v16qi __attribute__((vector_size(16)));
constexpr int mask() { v16qi v = {}; return __builtin_ia32_pmovmskb128(v); }
static_assert(mask() == 0);
