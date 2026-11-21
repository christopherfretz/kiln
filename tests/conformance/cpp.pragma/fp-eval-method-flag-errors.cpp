// clang `-ffp-eval-method=` (any value) with `#pragma clang fp
// reassociate(on)` or `reciprocal(on)`: the value-changing pragma is an
// error (`err_setting_eval_method_used_in_unsafe_context`, select
// "option 'ffp-eval-method'"); `(off)` is accepted. The negative twin of
// ir-fp-eval-method-flag.cpp (WP-1798).
//@ expect: compile-fail
//@ args: -std=c++26 -ffp-eval-method=source
//@ diag: 11:15 error "option 'ffp-eval-method' cannot be used with '#pragma clang fp reassociate'"
//@ diag: 15:15 error "option 'ffp-eval-method' cannot be used with '#pragma clang fp reciprocal'"
float f(float a, float b) {
#pragma clang fp reassociate(on)
  return a + b;
}
float g(float a, float b) {
#pragma clang fp reciprocal(on)
  return a / b;
}
float h(float a, float b) {
#pragma clang fp reassociate(off) reciprocal(off)
  return a * b;
}
