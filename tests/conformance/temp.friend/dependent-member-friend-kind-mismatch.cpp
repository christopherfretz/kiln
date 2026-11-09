// [temp.friend]/5, [dcl.type.elab]/5, [class.friend]/10: a friend of a
// member of every specialization of a class template names an entity of
// the kind it declares. A class friend naming a member function or static
// data member of the primary template, and a function friend naming a
// member class, are ill-formed (WP-1099; diagnosed as clang does for a
// member of the current instantiation, "redefinition ... as different kind
// of symbol"). clang accepts these with -Wunsupported-friend and turns off
// access control (an accepts-invalid; g++ 14 rejects each).
//@ skip(clang): clang accepts a mismatching [temp.friend]/5 friend with -Wunsupported-friend
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:42 error "redefinition of 'Fn' as different kind of symbol"
//@ diag: 20:15 note "previous definition is here"
//@ diag: 27:41 error "redefinition of 'Var' as different kind of symbol"
//@ diag: 21:14 note "previous definition is here"
//@ diag: 28:40 error "redefinition of 'Cls' as different kind of symbol"
//@ diag: 22:10 note "previous definition is here"
//@ diag: 29:42 warning "dependent nested name specifier 'A<U>' for friend class declaration is not supported; turning off access control for 'Outer'"
template <class T> struct A {
  static void Fn();
  static int Var;
  struct Cls {};
  struct Ok {};
};
struct Outer {
  template <class U> friend struct A<U>::Fn;
  template <class U> friend class A<U>::Var;
  template <class U> friend void A<U>::Cls();
  template <class U> friend struct A<U>::Ok;
};
