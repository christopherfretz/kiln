// [dcl.attr.deprecated]/4 (WP-297, clang delayed availability diagnostics):
// a use in the decl-specifiers, declarator or initializer of a declaration
// is warned only once the declaration is known not to be deprecated itself,
// for namespace-scope and member declarations alike.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 16:9 warning "'a' is deprecated"
//@ diag: 14:3 note "'a' has been explicitly marked deprecated here"
//@ diag: 18:32 warning "'S' is deprecated"
//@ diag: 17:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 18:70 warning "'S' is deprecated"
//@ diag: 17:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 22:14 warning "expression result unused"
[[deprecated]] int a;
[[deprecated]] int b = a;
int c = a, d [[deprecated]] = a;
struct [[deprecated]] S { int m; };
struct T { [[deprecated]] S s; S t; [[deprecated]] void f(S); void g(S); [[deprecated]] void h() { S x; } };
[[deprecated]] int (*fp)(S);
template<class X> struct [[deprecated]] P {};
template<> struct P<S> {};
void lam() { [] [[deprecated]] () { S z; }; }
