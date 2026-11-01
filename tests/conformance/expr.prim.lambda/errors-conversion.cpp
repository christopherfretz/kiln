// [expr.prim.lambda.closure]/8: the conversion is to a pointer to function
// with the call operator's parameter and return types only.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "errors-conversion.cpp:8:24)' to 'int (*)(int, int)'"
//@ diag: 8:24 note "candidate function"
void f() {
  int (*p)(int, int) = [](int) { return 0; };
}
