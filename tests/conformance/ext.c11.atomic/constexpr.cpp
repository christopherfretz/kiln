// Clang extension: the atomic operations are not constant expressions; the
// lock-free queries fold to constants.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 8:48 note "subexpression not valid in a constant expression"
//@ diag: 9:19 note "in call to 'f()'"
constexpr int f() { _Atomic(int) x = 1; return __c11_atomic_load(&x, 5); }
constexpr int k = f();
constexpr bool lf = __c11_atomic_is_lock_free(4) && __atomic_always_lock_free(8, nullptr);
static_assert(lf);
