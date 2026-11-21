// [basic.link]/14-16: an enumerator is not a TU-local entity (/16.1 lists
// types, functions, variables and templates), so naming an enumerator of a
// TU-local enumeration in an exported inline function is not an exposure.
// GCC 16.1 accepts each (it replaces an enumerator by its value); kiln
// follows the text and gives clang 22's warning at each use in an inline
// function's body (also in a lambda there and in a constexpr function) and
// in an inline variable's initializer, qualified names at their first
// token. No warning, as clang 22: an enumerator of an enumeration with
// linkage or of a local one, a function that is not inline, a default
// member initializer, a default argument, a template not instantiated.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 32:42 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 33:40 warning "TU local entity 'e2' is exposed [-WTU-local-entity-exposure]"
//@ diag: 34:41 warning "TU local entity 'a' is exposed [-WTU-local-entity-exposure]"
//@ diag: 35:56 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 36:57 warning "TU local entity 'e2' is exposed [-WTU-local-entity-exposure]"
//@ diag: 37:41 warning "TU local entity 'e2' is exposed [-WTU-local-entity-exposure]"
//@ diag: 38:52 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 39:46 warning "TU local entity 'e2' is exposed [-WTU-local-entity-exposure]"
//@ diag: 40:32 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
export module M;
namespace {
enum E { e1, e2 };
enum class F { a };
}
enum { Visible = 3 };
static int helper() { return 1; }

export inline int unqualified() { return e1; }
export inline int qualified() { return E::e2; }
export inline int scoped() { return int(F::a); }
export inline int local_constant() { constexpr int k = e1; return k; }
export inline int case_label(int x) { switch (x) { case e2: return 1; } return 0; }
export inline int array_bound() { int a[e2 + 1] = {}; return a[0]; }
export inline int in_lambda() { return [] { return e1; }(); }
export constexpr int constexpr_fn() { return e2; }
export inline int inline_var = e1;
export inline int linkage() { return Visible; }
export inline int local_enum() { enum { L1 }; return L1; }
export int not_inline() { return e1 + helper(); }
export struct X { int m = e1; int g(int = e2); };
export template <class T> inline int not_instantiated() { return e1; }
