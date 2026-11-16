// [temp.names]/6, [dcl.type.elab]/1: the name after `template` in an
// elaborated-type-specifier must be a template (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 10:27 error "'NT' following the 'template' keyword does not refer to a template"
//@ diag: 8:74 note "declared as a non-template here"
template<typename T> struct A { template<typename U> struct W {}; struct NT {}; };
void g() {
  struct A<int>::template NT<int> a;
}
