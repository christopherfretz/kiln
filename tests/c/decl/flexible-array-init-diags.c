// C23 6.7.2.1p20 (structure specifiers): initializing a flexible array
// member is a GNU extension clang warns about by default
// (-Wgnu-flexible-array-initializer); it is allowed only for an object of
// static storage duration at the top level of its initializer (an
// automatic one, or one nested in an array, is an error). The member
// itself is standard since C99 and draws no -Wc99-extensions.
//@ expect: compile-fail
//@ args: -pedantic
//@ strict-diags
//@ diag: 23:40 warning "flexible array initialization is a GNU extension"
//@ diag: 23:24 note "initialized flexible array member 's' is here"
//@ diag: 24:39 warning "flexible array initialization is a GNU extension"
//@ diag: 24:23 note "initialized flexible array member 'a' is here"
//@ diag: 24:61 warning "use of an empty initializer is a C23 extension"
//@ diag: 24:61 warning "flexible array initialization is a GNU extension"
//@ diag: 24:23 note "initialized flexible array member 'a' is here"
//@ diag: 24:61 warning "zero size arrays are an extension"
//@ diag: 25:13 warning "'struct G' may not be used as an array element due to flexible array member"
//@ diag: 25:26 error "initialization of flexible array member is not allowed"
//@ diag: 24:23 note "initialized flexible array member 'a' is here"
//@ diag: 27:21 error "initialization of flexible array member is not allowed"
//@ diag: 24:23 note "initialized flexible array member 'a' is here"
struct F { int n; char s[]; } f = { 1, "hey" };
struct G { int n; int a[]; } g = { 1, {1, 2, 3} }, e = { 2, {} };
struct G arr[1] = { { 1, {2} } };
void h(void) {
  struct G l = { 1, {2} };
  (void)l;
}
