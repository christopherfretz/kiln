// [expr.const]/10, GNU `__atomic_is_lock_free`: without an aligned typedef
// an 8-byte object of alignment 4 is not known to be lock-free, so the
// builtin is not a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:37 note "subexpression not valid in a constant expression"
//@ diag: 11:15 note "in call to 'f()'"
struct I2 { int i[2]; };
constexpr bool f() { I2 a{}; return __atomic_is_lock_free(sizeof(I2), &a); }
static_assert(f());
