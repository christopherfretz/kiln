// __builtin_has_attribute's rejected forms: a missing comma or attribute
// name, an attribute neither GCC nor clang knows, arguments checked as on
// a declaration (a dependent operand: has-attribute-dependent.cpp). Errors in
// clang's style; GCC rejects the same lines with its own wording. Found
// running GCC's C++ testsuite. WP-1611.
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
//@ diag: 24:33 error "expected ','"
//@ diag: 25:35 error "expected identifier"
//@ diag: 26:35 error "unknown attribute 'align'"
//@ diag: 27:35 error "unknown attribute 'aligned__'"
//@ diag: 28:35 error "requested alignment is not a power of 2"
//@ diag: 29:43 error "expression is not an integral constant expression"
//@ diag: 29:43 note "function parameter 'i' with unknown value cannot be used in a constant expression"
//@ diag: 22:11 note "declared here"
int v;

int f(int i) {
  int r = 0;
  r += __builtin_has_attribute(v);
  r += __builtin_has_attribute(v, 1);
  r += __builtin_has_attribute(v, align);
  r += __builtin_has_attribute(v, aligned__);
  r += __builtin_has_attribute(v, aligned(3));
  r += __builtin_has_attribute(v, aligned(i));
  return r;
}
// GCC 16.1 accepts these unknown-to-clang attribute names (answering from
// its own storage); kiln answers 0.
static_assert(!__builtin_has_attribute(v, nonstring));
