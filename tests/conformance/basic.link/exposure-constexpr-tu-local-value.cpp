// [basic.link]/15.2, /17: in a module interface unit's purview a constexpr
// variable that is not TU-local and is initialized to a TU-local value (a
// pointer to, or a reference bound to, a variable or function with
// internal linkage, or an object holding one, [basic.link]/16) is an
// exposure, and the program is ill-formed, exported or not (a
// non-exported namespace-scope constexpr variable of the purview has
// module linkage, [basic.link]/3.2.1). GCC 16.1 rejects each at the
// same line and column; clang 22 accepts (kiln's wording; WP-1669).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not diagnose a TU-local constexpr value
//@ strict-diags
//@ diag: 27:23 error "constexpr variable 'p' is initialized to a TU-local value: it exposes TU-local entity 'hidden'"
//@ diag: 28:23 error "constexpr variable 'r' is initialized to a TU-local value: it exposes TU-local entity 'hidden'"
//@ diag: 29:23 error "constexpr variable 'ap' is initialized to a TU-local value: it exposes TU-local entity 'in_anon'"
//@ diag: 30:24 error "constexpr variable 'fp' is initialized to a TU-local value: it exposes TU-local entity 'helper'"
//@ diag: 31:25 error "constexpr variable 's' is initialized to a TU-local value: it exposes TU-local entity 'hidden'"
//@ diag: 32:16 error "constexpr variable 'np' is initialized to a TU-local value: it exposes TU-local entity 'hidden'"
//@ diag: 33:41 error "constexpr variable 'm' is initialized to a TU-local value: it exposes TU-local entity 'hidden'"
export module M;
static int hidden;
namespace {
int in_anon;
}
static int helper() { return 1; }
struct Holder { int *a; };
export constexpr int *p = &hidden;
export constexpr int &r = hidden;
export constexpr int *ap = &in_anon;
export constexpr int (*fp)() = &helper;
export constexpr Holder s{&hidden};
constexpr int *np = &hidden;
export struct C { static constexpr int *m = &hidden; };
