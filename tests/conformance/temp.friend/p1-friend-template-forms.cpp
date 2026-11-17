// [temp.friend]/1: a friend class template is declared with an
// elaborated-type-specifier; [temp.expl.spec]/1: an explicit
// specialization is not declared by a friend declaration.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:21 error "friend type templates must use an elaborated type"
//@ diag: 16:19 error "friend type templates must use an elaborated type"
//@ diag: 19:26 error "cannot declare an explicit specialization in a friend"
//@ diag: 20:19 error "friend type templates must use an elaborated type"
//@ diag: 21:19 error "friend type templates must use an elaborated type"
template<int> void foo();
template<int T> struct S { struct X {}; struct Y {}; };
template<class T> struct W {
  template<class U> friend W<U>;
  template<int U> friend typename S<U>::Y;
};
struct A {
  template<> friend void foo<0>();
  template<int U> friend typename S<U>::Y;
  template<int U> friend S<U>;
};
W<int> w;
