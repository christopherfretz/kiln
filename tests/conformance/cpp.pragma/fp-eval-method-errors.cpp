// [cpp.predefined], clang's `#pragma clang fp eval_method`: inside a scope
// that contains the pragma (any method, `source` included)
// `__FLT_EVAL_METHOD__` is an error, with a note at the pragma; the scope
// ends with its compound statement, and at namespace scope it lasts to the
// end of the file. `reassociate(on)` and `reciprocal(on)` after the pragma
// in its scope are errors (clang `err_setting_eval_method_used_in_unsafe_context`);
// `off` is not (WP-1744).
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -std=c++26
//@ strict-diags
//@ diag: 27:11 error "'__FLT_EVAL_METHOD__' cannot be expanded inside a scope containing '#pragma clang fp eval_method'"
//@ diag: 26:15 note "#pragma entered here"
//@ diag: 29:13 error "'__FLT_EVAL_METHOD__' cannot be expanded inside a scope containing '#pragma clang fp eval_method'"
//@ diag: 26:15 note "#pragma entered here"
//@ diag: 36:13 error "'__FLT_EVAL_METHOD__' cannot be expanded inside a scope containing '#pragma clang fp eval_method'"
//@ diag: 35:15 note "#pragma entered here"
//@ diag: 42:15 error "'#pragma clang fp eval_method' cannot be used with '#pragma clang fp reassociate'"
//@ diag: 43:15 error "'#pragma clang fp eval_method' cannot be used with '#pragma clang fp reciprocal'"
//@ diag: 51:9 error "'__FLT_EVAL_METHOD__' cannot be expanded inside a scope containing '#pragma clang fp eval_method'"
//@ diag: 50:15 note "#pragma entered here"
//@ diag: 52:5 error "'__FLT_EVAL_METHOD__' cannot be expanded inside a scope containing '#pragma clang fp eval_method'"
//@ diag: 50:15 note "#pragma entered here"
int a = __FLT_EVAL_METHOD__;
void f() {
#pragma clang fp eval_method(double)
  int b = __FLT_EVAL_METHOD__;
  {
    int c = __FLT_EVAL_METHOD__;
  }
}
void g() {
  int d = __FLT_EVAL_METHOD__;
  {
#pragma clang fp eval_method(source)
    int e = __FLT_EVAL_METHOD__;
  }
  int h = __FLT_EVAL_METHOD__;
}
void k() {
#pragma clang fp eval_method(double)
#pragma clang fp reassociate(on)
#pragma clang fp reciprocal(on)
#pragma clang fp reassociate(off)
#pragma clang fp reciprocal(off)
}
void m() {
#pragma clang fp reassociate(on)
}
#pragma clang fp eval_method(extended)
int z = __FLT_EVAL_METHOD__;
#if __FLT_EVAL_METHOD__ == 0
#endif
