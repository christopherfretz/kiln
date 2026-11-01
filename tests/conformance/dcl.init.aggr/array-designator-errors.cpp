// [dcl.init.aggr]/3.1 (GNU array designators, clang
// `ActOnDesignatedInitializer` / `CheckDesignatedInitializer`): an array
// designator's index is an integral constant expression of integral or
// unscoped enumeration type, not negative, within the array bound; a range
// `[a ... b]` is not empty (diagnosed at the `...`) and is a
// -Wgnu-designator extension under -pedantic. An invalid designation drops
// the whole braced list (no conversion of `2.5` on line 2 is diagnosed).
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic
//@ diag: 31:14 warning "array designators are a C99 extension"
//@ diag: 31:15 error "array designator index (5) exceeds array bounds (3)"
//@ diag: 32:15 error "array designator value '-1' is negative"
//@ diag: 34:15 error "expression is not an integral constant expression"
//@ diag: 34:15 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 33:5 note "declared here"
//@ diag: 35:16 warning "use of GNU array range extension"
//@ diag: 35:16 error "array designator range [3, 1] is empty"
//@ diag: 36:17 warning "use of GNU array range extension"
//@ diag: 36:14 warning "array designators are a C99 extension"
//@ diag: 36:21 error "array designator index (5) exceeds array bounds (3)"
//@ diag: 37:14 error "integral constant expression must have integral or unscoped enumeration type, not 'double'"
//@ diag: 39:16 error "integral constant expression must have integral or unscoped enumeration type, not 'E'"
//@ diag: 41:17 error "array designator value '-2' is negative"
//@ diag: 42:44 warning "use of GNU array range extension"
//@ diag: 42:41 warning "array designators are a C99 extension"
//@ diag: 44:18 warning "use of GNU array range extension"
//@ diag: 44:15 warning "array designators are a C99 extension"
//@ diag: 42:44 error "array designator range [1, 0] is empty"
//@ diag: 43:9 note "in instantiation of function template specialization 'ft<1>' requested here"
int a1[3] = {[5] = 1};
int a2[3] = {[-1] = 1, 2.5};
int n = 2;
int a3[3] = {[n] = 1};
int a5[] = {[3 ... 1] = 1};
int a6[3] = {[0 ... 5] = 1};
int a9[] = {[1.5] = 1};
enum class E { e };
int a10[2] = {[E::e] = 1};
constexpr int k = -2;
int a11[2] = {{[k] = 1}};
template <int I> int ft() { int a[2] = {[I ... 0] = 1}; return a[0]; }
int u = ft<1>();
int a12[2] = {[0 ... 1] = 1};
