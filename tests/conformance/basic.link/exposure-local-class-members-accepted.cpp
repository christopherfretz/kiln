// [basic.link]/15.1, [class.mfct]/1: a member function defined in a local
// class attached to a named module is not inline, so /15.1 ignores its body
// even in an inline function: naming a TU-local entity there is not an
// exposure. GCC 16.1 accepts; kiln follows the text and gives clang 22's
// warning at each use. A local class of a function that is not inline, or
// of a TU-local one, is not checked at all (no warning, as clang 22). The
// exposures in a local class are in exposure-local-class-members.cpp.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 25:56 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
//@ diag: 26:60 warning "TU local entity 'hidden' is exposed [-WTU-local-entity-exposure]"
//@ diag: 28:42 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
//@ diag: 28:53 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 32:43 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"
export module M;
static int hidden;
static int helper() { return 1; }
namespace {
struct S { int v; };
enum E { e1 };
}

export inline int body() { struct L { int g() { return helper(); } }; return L().g(); }
export inline int variable() { struct L { int g() { return hidden; } }; return L().g(); }
export inline int nested() {
  struct L { struct N { int g() { return helper() + e1; } }; };
  return L::N().g();
}
export inline int in_lambda() {
  return [] { struct L { int g() { return helper(); } }; return L().g(); }();
}
export int not_inline() {
  struct L : S { S s; int g(S) { return helper(); } };
  return 0;
}
static inline int tu_local() {
  struct L : S { int g() { return helper(); } };
  return L().g();
}
export inline int uses_tu_local() { return 0; }
export inline int nothing() { struct L { int g() { return 1; } }; return L().g(); }
