// [dcl.stc]/2: -Wno-register makes `register` acceptable in C++17 (clang's
// ext_register_storage_class is a default-error warning) (WP-1467).
//@ std: c++17
//@ args: -Wno-register
//@ strict-diags
int f(register int a) {
  register int x = a;
  return x;
}
