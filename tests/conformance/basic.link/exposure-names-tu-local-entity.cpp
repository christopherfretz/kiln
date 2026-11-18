// [basic.link]/15.1, /17: a declaration in a module interface that names
// a TU-local entity outside the parts /15 ignores is an exposure: an
// inline function's body (exported or not), a function's return type, a
// constexpr local of an inline function. GCC 16.1 rejects each; clang 22
// warns (-WTU-local-entity-exposure). kiln does not check names yet.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ xfail: WP-1651 remainder: exposures by naming a TU-local entity are not diagnosed
//@ diag: 18:19 error "exposes TU-local entity"
//@ diag: 19:12 error "exposes TU-local entity"
//@ diag: 20:17 error "exposes TU-local entity"
//@ diag: 21:20 error "exposes TU-local entity"
export module M;
static int helper() { return 1; }
static int target;
namespace { struct S {}; }
export inline int f() { return helper(); }
inline int g() { return helper(); }
export inline S mk() { return {}; }
export inline int *lp() { constexpr int *p = &target; return p; }
