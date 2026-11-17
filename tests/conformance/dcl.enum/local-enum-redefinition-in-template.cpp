// [dcl.enum]/3, [basic.def.odr]/1: the rejected twin of
// local-opaque-enum-in-template.cpp: two enum-specifiers of one
// block-scope enumeration in a function template are a redefinition,
// reported at the definition (once). WP-1613.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:8 error "redefinition of 'J'"
//@ diag: 10:8 note "previous definition is here"
template <class T> int g() {
  enum J : T { D = 1 };
  enum J : T { E = 2 };
  return D;
}
int a = g<int>();
