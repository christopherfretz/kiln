// [basic.def.odr]/5,/10: a potentially-evaluated splice-expression
// designating a local entity names and odr-uses it; from a function
// other than the local's own (here a template given `^^b` as its
// argument, or a member of a local class) the local is not odr-usable
// and the program is ill-formed. Unevaluated (`sizeof`) and a constexpr
// local whose value is read are fine. Found running GCC's C++
// testsuite; GCC 16.1 agrees on every line.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 23:49 error "use of local variable 'b' with automatic storage from function 'g'"
//@ diag: 31:3 note "in instantiation of function template specialization 'use<^^b>' requested here"
//@ diag: 28:7 note "'b' declared here"
//@ diag: 23:49 error "use of local variable 'p' with automatic storage from function 'g'"
//@ diag: 32:3 note "in instantiation of function template specialization 'use<^^p>' requested here"
//@ diag: 27:12 note "'p' declared here"
//@ diag: 30:31 error "reference to local variable 'b' declared in enclosing function 'g'"
//@ diag: 28:7 note "'b' declared here"
#include <meta>

template <std::meta::info R> int use() { return [:R:]; }
template <std::meta::info R> int size() { return sizeof([:R:]); }
template <std::meta::info R> int own() { int x = 1; return [:^^x:] + [:R:]; }

void g(int p) {
  int b = 1;
  constexpr int c = 2;
  struct L { int h() { return [:^^b:]; } };
  use<^^b>();
  use<^^p>();
  use<^^c>();
  size<^^b>();
  own<^^c>();
}
