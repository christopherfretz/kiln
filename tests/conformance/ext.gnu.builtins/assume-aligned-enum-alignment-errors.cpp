// The enumerator alignment of __builtin_assume_aligned is still checked: a
// scoped enumeration is not an integer constant (clang
// `BuiltinConstantArg`), and an unscoped one must be a power of 2.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++17
//@ diag: 11:48 error "argument to '__builtin_assume_aligned' must be a constant integer"
//@ diag: 12:48 error "requested alignment is not a power of 2"
enum class Scoped { V = 8 };
enum { Three = 3 };
const void *f(const void *p) { return (void)0, __builtin_assume_aligned(p, Scoped::V); }
const void *g(const void *p) { return (void)0, __builtin_assume_aligned(p, Three); }
