// [dcl.type.auto.deduct]/4, [temp.arg.general]/1: a variable's or return
// type's constrained placeholder names the concept-id with the deduced
// type first; clang 22 checks that list when the placeholder is deduced,
// so in a template the arity error comes at the instantiation whose pack
// makes it too short or too long (at the `auto`), and an instantiation
// where it fits is valid (clang 19 gives these errors no location).
// WP-1610.
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 22:23 error "too few template arguments for concept 'G'"
//@ diag: 26:24 note "in instantiation of member function 'S<int>::f' requested here"
//@ diag: 20:37 note "template is declared here"
//@ diag: 23:12 error "too many template arguments for concept 'G'"
//@ diag: 27:34 note "in instantiation of member function 'S<int, int, int>::g' requested here"
//@ diag: 20:37 note "template is declared here"
//@ diag: 28:13 error "too many template arguments for concept 'G'"
//@ diag: 20:37 note "template is declared here"
template <class T, class U> concept G = true;
template <class T, class... Us> struct S {
  void f() { G<Us...> auto y = T(); }
  G<Us...> auto g() { return T(); }
};
void ok() { S<int, int>{}.f(); S<int, int>{}.g(); }
void bad1() { S<int>{}.f(); }
void bad2() { S<int, int, int>{}.g(); }
G<int, int> auto h() { return 1; }
