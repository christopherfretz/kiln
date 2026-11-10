// [temp.inst], [dcl.init.aggr]/3.1: a designated initializer list in a template
// whose type is not dependent is checked in the definition and again at each
// instantiation (clang rebuilds it), so the order and override warnings repeat under
// the instantiation note, in every context the list appears in. The C99-extension
// warnings belong to the written designators and fire once. Ranges cover the clause.
//@ skip(gcc): relies on clang's C99 designator extensions (out-of-order, nested, array and mixed designators)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 47:23 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 47:15 note "previous initialization for field 'b' is here"
//@ diag: 48:10 warning "nested designators are a C99 extension"
//@ diag: 49:23 warning "initializer overrides prior initialization of this subobject"
//@ diag: 49:15 note "previous initialization is here"
//@ diag: 50:17 warning "array designators are a C99 extension"
//@ diag: 51:21 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 51:13 note "previous initialization for field 'b' is here"
//@ diag: 52:24 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 52:16 note "previous initialization for field 'b' is here"
//@ diag: 57:9 warning "nested designators are a C99 extension"
//@ diag: 57:14 warning "array designators are a C99 extension"
//@ diag: 58:12 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 58:9 note "first non-designated initializer is here"
//@ diag: 59:9 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 59:17 note "first non-designated initializer is here"
//@ diag: 47:23 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 55:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 47:15 note "previous initialization for field 'b' is here"
//@ diag: 49:23 warning "initializer overrides prior initialization of this subobject"
//@ diag: 49:15 note "previous initialization is here"
//@ diag: 51:21 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 51:13 note "previous initialization for field 'b' is here"
//@ diag: 52:24 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 52:16 note "previous initialization for field 'b' is here"
//@ diag: 47:23 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 56:9 note "in instantiation of function template specialization 'f<long>' requested here"
//@ diag: 47:15 note "previous initialization for field 'b' is here"
//@ diag: 49:23 warning "initializer overrides prior initialization of this subobject"
//@ diag: 49:15 note "previous initialization is here"
//@ diag: 51:21 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 51:13 note "previous initialization for field 'b' is here"
//@ diag: 52:24 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 52:16 note "previous initialization for field 'b' is here"
struct S { int a, b; };
struct N { S s; int c; int arr[4]; };
void use(S);
template <class T> int f() {
  S x = {.b = 1, .a = 2};
  N n = {.s.a = 1, .c = 2};
  S y = {.a = 1, .a = 2};
  int arr[2] = {[1] = 1};
  use({.b = 1, .a = 2});
  (void)S{.b = 1, .a = 2};
  return 0;
}
int i = f<int>();
int j = f<long>();
N n1 = {.arr [ 2 ] = 1};
S s1 = {1, .b = 2};
S s2 = {.a = 1, 2};
