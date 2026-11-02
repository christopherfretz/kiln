// [namespace.udecl]/1, [temp.res.general]/1: an error in the substituted
// nested-name-specifier of a dependent using-declaration (block scope,
// class member, `using typename`, a pack expansion) is reported where the
// nested-name-specifier begins, as is "not a base class", which names the
// class specialization (clang QualifierLoc.getBeginLoc(); WP-328: kiln
// reported at the declarator's name). Name lookup errors stay at the name.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 26:9 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 31:9 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:9 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 36:31 error "base specifier must name a class"
//@ diag: 40:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 37:9 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 38:18 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 42:37 error "using declaration refers into 'X::', which is not a base class of 'D<X>'"
//@ diag: 44:6 note "in instantiation of template class 'D<X>' requested here"
//@ diag: 45:34 error "base specifier must name a class"
//@ diag: 48:8 note "in instantiation of template class 'G<int>' requested here"
//@ diag: 46:9 error "type 'int' cannot be used prior to '::' because it has no members"
//@ requires: clang<22
// clang 22 emits error: using declaration refers into 'X', which is not a base class of 'D<X>'; WP-355 residual
template <class T> int f() {
  using T::member;
  return 0;
}
int x = f<int>();
template <class T> int g() {
  using T::B::member;
  return 0;
}
int y = g<int>();

template <class T> struct C : T {
  using T::member;
  using typename T::type;
};
C<int> c;
struct X { int member; };
template <class T> struct D { using T::member; };

D<X> d;
template <class... T> struct G : T... {
  using T::member...;
};
G<int> gi;
