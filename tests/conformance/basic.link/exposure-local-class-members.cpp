// [basic.link]/14-17: the declarations of a local class of an inline
// function are part of that function's body, which /15.1 does not ignore.
// In a module interface, naming a TU-local entity in them is an exposure:
// a base class or a data member's type (the error at the class), a member
// function's parameter or return type, the body of a member function that
// is inline (written `inline` or `constexpr`; the error at the member),
// also in a nested class and in a local class of a lambda there. GCC 16.1
// rejects each (at the same declarations); clang 22 only warns for the
// member function bodies (a deviation, docs/status/deviations.md). A member
// function defined in a local class attached to a named module is not
// inline ([class.mfct]/1): its body is ignored
// (exposure-local-class-members-accepted.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 34:35 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 35:37 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 36:48 error "declaration of 'g' exposes TU-local entity 'S'"
//@ diag: 37:43 error "declaration of 'g' exposes TU-local entity 'S'"
//@ diag: 38:57 error "declaration of 'g' exposes TU-local entity 'helper'"
//@ diag: 40:28 error "declaration of 'g' exposes TU-local entity 'twice'"
//@ diag: 43:48 error "declaration of 'N' exposes TU-local entity 'S'"
//@ diag: 44:52 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 45:55 error "declaration of 'g' exposes TU-local entity 'hidden'"
export module M;
static int hidden;
static int helper() { return 1; }
static constexpr int twice(int x) { return 2 * x; }
namespace {
struct S { int v; };
}

export inline int base() { struct L : S { int g(); }; return 0; }
export inline int member() { struct L { S s; }; return 0; }
export inline int parameter() { struct L { int g(S); }; return 0; }
export inline int result() { struct L { S g(); }; return 0; }
export inline int inline_body() { struct L { inline int g() { return helper(); } }; return L().g(); }
export inline int constexpr_body() {
  struct L { constexpr int g(int x) { return twice(x); } };
  return L().g(1);
}
export inline int nested() { struct L { struct N { S s; }; }; return 0; }
export inline int in_lambda() { return [] { struct L { S s; }; return 0; }(); }
export inline int *address() { struct L { inline int *g() { return &hidden; } }; return L().g(); }
