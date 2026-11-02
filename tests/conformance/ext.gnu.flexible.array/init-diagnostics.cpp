// Initialization of a flexible array member (spec/sema.md §5.8; clang
// `CheckFlexibleArrayInit`): an empty list is a GNU extension (and a
// zero-length array); elements are an error in a local, a temporary or a
// nested subobject. A flexible array member does not decay in a constant
// expression.
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic
//@ diag: 30:24 warning "flexible array members are a C99 feature"
//@ diag: 31:21 warning "'a' may not be nested in a struct due to flexible array member"
//@ diag: 33:12 warning "flexible array initialization is a GNU extension"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 33:12 warning "zero size arrays are an extension"
//@ diag: 35:14 error "initialization of flexible array member is not allowed"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 36:14 warning "flexible array initialization is a GNU extension"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 36:14 warning "zero size arrays are an extension"
//@ diag: 38:21 warning "flexible array initialization is a GNU extension"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 38:21 warning "zero size arrays are an extension"
//@ diag: 39:17 error "initialization of flexible array member is not allowed"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 40:14 error "initialization of flexible array member is not allowed"
//@ diag: 30:24 note "initialized flexible array member 'd' is here"
//@ diag: 43:23 warning "flexible array members are a C99 feature"
//@ diag: 45:15 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 44:36 note "array-to-pointer decay of array member without known bound is not supported"
//@ diag: 45:19 note "in call to 'g()'"
struct A { int n; char d[]; };
struct W { int x; A a; };
A g0 = {1};
A g1 = {1, {}};
void f() {
  A l1 = {1, {2}};
  A l2 = {1, {}};
  A l3 = {1};
  static A s1 = {1, {}};
  W w = {1, {2, {3}}};
  (void)A{1, {4}};
  (void)l2; (void)l3;
}
struct K { int n; int d[]; };
constexpr int g() { K k{1}; return k.d[0]; }
constexpr int h = g();
