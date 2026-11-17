// [namespace.udecl]/10: a using-declarator naming an enumerator through
// its enumeration cannot be repeated in a class scope.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "redeclaration of using declaration"
//@ diag: 11:12 note "previous using declaration"
enum E { x };
enum class K { z, w };
struct S {
  using E::x;
  using E::x;
  using K::z;
  using K::w;
};
