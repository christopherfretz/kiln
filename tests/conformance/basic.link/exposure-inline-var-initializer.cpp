// [basic.link]/15.1.2: "the initializer for a variable or variable
// template" is ignored when looking for an exposure, and /17 makes only a
// constexpr variable's TU-local value one. So in a module interface an
// inline variable (not constexpr) whose initializer names a TU-local
// entity is valid: a pointer to or a reference bound to a variable with
// internal linkage, a pointer to a static function, a call to one, a call
// in a lambda, exported or not. kiln follows the text and clang 22, and
// gives clang 22's warning at each use. GCC 16.1 rejects the ones
// constant-initialized to a TU-local value (docs/status/deviations.md).
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ skip(gcc): GCC 16.1 rejects ip, ir, nonexported and fp
//@ strict-diags
//@ diag: 25:26 warning "TU local entity 'hidden' is exposed [-WTU-local-entity-exposure]"
//@ diag: 26:25 warning "TU local entity 'hidden' is exposed [-WTU-local-entity-exposure]"
//@ diag: 27:28 warning "TU local entity 'hidden' is exposed [-WTU-local-entity-exposure]"
//@ diag: 28:30 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
//@ diag: 29:29 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
//@ diag: 30:44 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
export module M;
static int hidden;
static int helper() { return 1; }
static const int K = 3;
export inline int *ip = &hidden;
export inline int &ir = hidden;
inline int *nonexported = &hidden;
export inline int (*fp)() = &helper;
export inline int dynamic = helper();
export inline int via_lambda = [] { return helper(); }();
export int *nip = &hidden;
export const int cp = K;
export inline int folded = K + 1;
