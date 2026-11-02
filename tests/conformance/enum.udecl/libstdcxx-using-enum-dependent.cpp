// [enum.udecl]/1: the using-enum-declarator designates a non-dependent
// type; a member enumeration of the current instantiation and a local
// enumeration of a function template are dependent (companion of
// libstdcxx-local-using-enum-template.cpp, WP-323). Verified against
// clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:24 error "using-enum cannot name a dependent type"
//@ diag: 15:38 error "use of undeclared identifier 'y'"
//@ diag: 20:14 error "using-enum cannot name a dependent type"
//@ diag: 21:14 error "use of undeclared identifier 'q'"

template<class T> struct S {
  enum class Inner { x, y };
  int g() { using enum Inner; return y == Inner::y ? 7 : 0; }
};

template<class T> int h() {
  enum class L { p, q };
  using enum L;
  return int(q) + T(1);
}
