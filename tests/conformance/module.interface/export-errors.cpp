// [module.interface]/1-3: export placement and what may be exported
// (diagnostics and locations as clang 22).
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 17:3 error "export declaration appears within another export declaration"
//@ diag: 16:1 note "export block begins here"
//@ diag: 20:1 error "export declaration appears within anonymous namespace"
//@ diag: 19:11 note "anonymous namespace begins here"
//@ diag: 22:18 error "anonymous namespaces cannot be exported"
//@ diag: 27:19 error "declaration of 'f' with internal linkage cannot be exported"
//@ diag: 29:14 error "declaration of 'g' with internal linkage cannot be exported"
//@ diag: 28:1 note "export block begins here"
export module M;
export {
  export int a;
}
namespace {
export int b;
}
export namespace {
int c;
}
static int d;
export int e();
export static int f();
export {
  static int g;
}
