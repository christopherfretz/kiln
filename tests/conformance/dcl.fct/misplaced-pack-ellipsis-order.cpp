// [dcl.fct]/20, [temp.param]/17, [dcl.decl.general]/4: a misplaced pack
// `...` after a parameter's declarator is diagnosed while the declarator
// is parsed (clang's DiagnoseMisplacedEllipsisInDeclarator), so it comes
// before any diagnostic of the parameter's type or specifiers, for
// function parameters and non-type template parameters alike.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 18:43 error "'...' must immediately precede declared identifier"
//@ diag: 18:32 error "invalid storage class specifier in function declarator"
//@ diag: 19:38 error "'...' must immediately precede declared identifier"
//@ diag: 21:23 error "'...' must immediately precede declared identifier"
//@ diag: 21:10 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 22:19 error "'...' must immediately precede declared identifier"
//@ diag: 22:17 error "'a' declared as a pointer to a reference of type 'Ts &'"
//@ diag: 23:22 error "'...' must immediately precede declared identifier"
//@ diag: 23:19 error "'a' declared as an array with a negative size"
template <class... Ts> void f1(static Ts a...);
template <class... Ts> void f2(Ts a[]...);
template <class... Ts> struct S {
  void m(register Ts a...);
  template <Ts &*a...> struct A;
  template <int a[-1]...> struct B;
};
