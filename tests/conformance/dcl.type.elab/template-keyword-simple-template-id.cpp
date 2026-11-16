// [dcl.type.elab]/1: elaborated-type-specifier `class-key
// nested-name-specifier template(opt) simple-template-id`
// (`struct A<int>::template W<int> w2;`). Reduced from GCC's
// g++.dg/template/dependent-name11.C; found running GCC's C++ testsuite
// (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
template<typename T> struct A { template<typename U> struct W {}; };
void g() {
  struct A<int>::W<int> w;
  struct A<int>::template W<int> w2;
}
