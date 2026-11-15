// `__builtin_sponentry()` (AArch64, clang BuiltinsAArch64.def: `void *()`,
// `Const`, not `NoThrow`): no arguments; its value unused is warned
// (-Wunused-value, the const attribute); never a constant expression.
// Found in unwinder-style code. WP-1428.
//@ args: --target=aarch64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:45 error "too many arguments to function call, expected 0, have 1"
//@ diag: 17:21 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 18:12 warning "ignoring return value of function declared with const attribute"
#if !__has_builtin(__builtin_sponentry)
#error __builtin_sponentry is an AArch64 builtin
#endif
static_assert(__is_same(decltype(__builtin_sponentry()), void *));
static_assert(!noexcept(__builtin_sponentry()));
void *s(int i) { return __builtin_sponentry(i); }
constexpr void *c = __builtin_sponentry();
void u() { __builtin_sponentry(); }
