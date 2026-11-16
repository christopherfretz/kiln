// [class.ctor.general]/1, [dcl.ambig.res]: in a friend declaration a name
// with a type specifier never declares a constructor (`friend void S(T);`
// in `S` befriends a namespace-scope function named `S`), and an
// unqualified name after a dependent qualifier in the parentheses is not
// a type (`friend C(T::fn)();` befriends `T::fn` returning `C`). GCC 16.1
// and clang 22. Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
struct S {
  struct T {};
  friend void S(T);
};
struct S2 {
  struct T {};
  template<typename> friend void S2(T);
};
template<typename T> struct C {
  friend C(T::fn)();
  friend C(T::fn)(int, int);
};
struct X {};
template<typename T> struct B {
  friend X(T::fn)();
  friend X(T::fn)(int);
};
