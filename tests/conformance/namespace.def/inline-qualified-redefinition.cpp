// [namespace.def.general]/7, [dcl.meaning]/3: a qualified variable
// declarator whose redeclaration lookup finds more than one entity (a
// member of the namespace and one of an inline namespace in it) is
// clang's ambiguous redeclaration: "redefinition ... as different kind of
// symbol", with the note at the first-declared entity. One entity found
// through an inline namespace is redeclared normally. WP-235.
//@ expect: compile-fail
//@ strict-diags
// Verified against clang 19 (WP-235).
//@ diag: 21:8 error "redefinition of 't' as different kind of symbol"
//@ diag: 20:40 note "previous definition is here"
//@ diag: 23:8 error "redefinition of 't' as different kind of symbol"
//@ diag: 22:47 note "previous definition is here"
//@ diag: 25:8 error "redefinition of 't' as different kind of symbol"
//@ diag: 24:19 note "previous definition is here"
//@ diag: 27:8 error "redefinition of 't'"
//@ diag: 26:40 note "previous definition is here"
//@ diag: 31:15 error "redefinition of 'u' as different kind of symbol"
//@ diag: 30:40 note "previous definition is here"
namespace Z { inline namespace I { int t; } int t; }
int Z::t;
namespace Y { inline namespace I { extern int t; } extern int t; }
int Y::t;
namespace C { int t; inline namespace I { int t; } }
int C::t;
namespace D { inline namespace I { int t; } }
int D::t;
namespace E { inline namespace I { extern int t; } }
int E::t;
namespace F { inline namespace I { int u; } extern int u; }
extern int F::u;
namespace G { inline namespace I { void f(); } void f(int); }
void G::f() {}
void G::f(int) {}
namespace H { inline namespace I { int v; } }
namespace H { inline namespace I { extern int v; } }
extern int H::v;
