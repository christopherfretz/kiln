// clang address_space(N) and opencl_* attributes (spec/sema.md "Address
// spaces"; WP-1803): where a space may not be written, and a malformed
// address_space argument. clang HandleAddressSpaceTypeAttribute,
// CheckVariableDeclarationType wording.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:50 error "automatic variable qualified with an address space"
//@ diag: 23:46 error "parameter may not be qualified with an address space"
//@ diag: 24:50 error "multiple address spaces specified for type"
//@ diag: 25:50 warning "multiple identical address spaces specified for type"
//@ diag: 26:16 error "address space is negative"
//@ diag: 28:16 error "'address_space' attribute requires an integer constant"
//@ diag: 29:16 error "'address_space' attribute takes one argument"
//@ diag: 30:16 error "'address_space' attribute takes one argument"
//@ diag: 31:16 error "'opencl_global' attribute takes no arguments"
//@ diag: 32:50 error "field may not be qualified with an address space"
//@ diag: 36:5 warning "to a declaration is deprecated; apply it to the type instead"
//@ diag: 36:31 error "automatic variable qualified with an address space"
//@ diag: 40:3 warning "to a declaration is deprecated; apply it to the type instead"

void d() { __attribute__((address_space(1))) int x; }
__attribute__((address_space(1))) int e();
void f(__attribute__((address_space(1))) int x);
__attribute__((address_space(1))) __attribute__((address_space(2))) int g;
__attribute__((address_space(1))) __attribute__((address_space(1))) int g2;
__attribute__((address_space(-1))) int h;
int n;
__attribute__((address_space(n))) int h5;
__attribute__((address_space)) int h6;
__attribute__((address_space(1, 2))) int h7;
__attribute__((opencl_global(1))) int h8;
struct S { __attribute__((address_space(1))) int m; };
void l() {
  static __attribute__((address_space(1))) int sx;
  __attribute__((address_space(1))) int *pp = nullptr;
  [[clang::opencl_local]] int bad;
  extern __attribute__((opencl_global)) int ok;
}
__attribute__((address_space(3))) int ok3;
[[clang::address_space(4)]] int ok4;
