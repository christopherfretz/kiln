// [dcl.attr.likelihood] (WP-203): as in clang 19, the if/else likelihood
// conflict (reported straight to the engine) is repeated for each
// instantiation without instantiation notes, while the incompatible
// pair in one sequence is diagnosed once, in the template definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:12 warning "conflicting attributes 'likely' are ignored"
//@ diag: 16:31 note "conflicting attribute is here"
//@ diag: 17:23 error "'unlikely' and 'likely' attributes are not compatible"
//@ diag: 17:12 note "conflicting attribute is here"
//@ diag: 16:12 warning "conflicting attributes 'likely' are ignored"
//@ diag: 16:31 note "conflicting attribute is here"
//@ diag: 16:12 warning "conflicting attributes 'likely' are ignored"
//@ diag: 16:31 note "conflicting attribute is here"
template <class T> int g(T x) {
  if (x) [[likely]] {} else [[likely]] {}
  if (x) [[likely]] [[unlikely]] {}
  return 0;
}
int h() { return g(1) + g(2L); }
