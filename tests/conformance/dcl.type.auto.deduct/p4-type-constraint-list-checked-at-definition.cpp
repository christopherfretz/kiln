// [dcl.type.auto.deduct]/4, [temp.arg.general]/1, [temp.variadic]/10: a
// variable's constrained placeholder names the concept-id with the
// deduced type first. In a template, with an initializer that is not
// type-dependent, clang 22 deduces the placeholder at the definition and
// checks that list there, even when the type-constraint's arguments are
// dependent: a pack expansion onto a non-pack parameter, or too many
// arguments, is reported once, at the argument or the concept's name,
// whether or not the template is instantiated. Satisfaction waits for the
// arguments, and a type-dependent initializer waits for the instantiation
// (p4-type-constraint-list-checked-at-deduction.cpp). WP-1629.
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 29:40 error "too many template arguments for concept 'G'"
//@ diag: 27:37 note "template is declared here"
//@ diag: 30:38 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 27:26 note "template parameter is declared here"
//@ diag: 31:38 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 28:26 note "template parameter is declared here"
//@ diag: 32:50 error "too many template arguments for concept 'G'"
//@ diag: 27:37 note "template is declared here"
//@ diag: 33:47 error "too many template arguments for concept 'G'"
//@ diag: 27:37 note "template is declared here"
//@ diag: 34:43 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 27:26 note "template parameter is declared here"
template <class T, class U> concept G = true;
template <class T, class U, class... V> concept F = sizeof(T) < 100;
template <class T> void f3() { G<T, T> auto v = 0; (void)v; }
template <class... Us> void f2() { G<Us...> auto v = 0; (void)v; }
template <class... Us> void f1() { F<Us...> auto v = 0; (void)v; }
struct S { template <class T> void m() { G<T, T> auto v = 0; (void)v; } };
template <class T> void f5() { static G<T, T> auto v = 0; (void)v; }
template <class... Us> void f6() { [] { G<Us...> auto v = 0; (void)v; }; }
// Valid: the list fits; a dependent initializer.
template <class T> void ok1() { G<T> auto v = 0; F<T, T> auto w = 0; (void)v; (void)w; }
template <class... Us> void ok2() { F<int, Us...> auto v = 0; (void)v; }
template <class T> void ok3() { G<T, T, T> auto v = T(); (void)v; }
void use() { f3<int>(); f2<>(); S().m<int>(); f5<int>(); ok1<int>(); ok2<>(); }
