// [expr.typeid], [except.handle], [except.throw]: a variably modified
// type (the VLA extension) has no std::type_info, so 'typeid' of one, a
// handler of one and a throw of a pointer to a VLA are errors, as clang's
// (the last through the exception object type 'int (*)[*]', clang
// getExceptionObjectType). kiln accepted all three and then failed in
// code generation. A 1-D VLA handler or thrown operand decays to 'int *'
// and is fine. Verified against clang 19 and 22.
//@ expect: compile-fail
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 22:9 error "'typeid' of variably modified type 'int[n]'"
//@ diag: 23:9 error "'typeid' of variably modified type 'int[n][2]'"
//@ diag: 24:9 error "'typeid' of variably modified type 'int (*)[n]'"
//@ diag: 25:9 error "'typeid' of variably modified type 'int (*)[n]'"
//@ diag: 33:18 error "cannot catch variably modified type 'int (*)[n]'"
//@ diag: 36:18 error "cannot catch variably modified type 'int (&)[n]'"
//@ diag: 47:9 error "cannot initialize an exception object of type 'int (*)[*]' with an rvalue of type 'int[n][n]'"
namespace std { class type_info; }

void typeid_vla(int n) {
  int a[n];
  (void)typeid(a);
  (void)typeid(int[n][2]);
  (void)typeid(&a);
  (void)typeid(int (*)[n]);
}

void catch_vla(int n) {
  try {
  } catch (int a[n]) { // decays to int *: fine
  }
  try {
  } catch (int (*p)[n]) {
  }
  try {
  } catch (int (&r)[n]) {
  }
}

void throw_vla(int n) {
  int a[n];
  throw a; // decays to int *: fine
}

void throw_vla_2d(int n) {
  int a[n][n];
  throw a;
}
