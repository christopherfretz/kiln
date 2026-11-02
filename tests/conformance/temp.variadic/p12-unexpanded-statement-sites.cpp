// [temp.variadic]/12: a pack named outside an expansion in a template's
// statements is diagnosed when the template is defined: a switch
// condition, a range and a case value as "expression" at the operand's
// begin, an exception-declaration's type at the declarator-id, and a
// lambda-expression whose parameter's default argument names the pack
// at the lambda's full-expression.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ args: -Wno-unused-value
//@ diag: 20:11 error "expression contains unexpanded parameter pack 't'"
//@ diag: 21:18 error "exception type contains unexpanded parameter pack 'T'"
//@ diag: 22:20 error "exception type contains unexpanded parameter pack 'T'"
//@ diag: 23:3 error "expression contains unexpanded parameter pack 't'"
//@ diag: 24:16 error "expression contains unexpanded parameter pack 't'"
//@ diag: 25:17 error "static assertion contains unexpanded parameter pack 't'"
//@ diag: 27:49 error "expression contains unexpanded parameter pack 'N'"
//@ diag: 28:55 error "expression contains unexpanded parameter pack 'N'"
template<class... T> void f(T... t) {
  switch (t) {}
  try {} catch (T) {}
  try {} catch (T &e) {}
  [](int = sizeof(t)) {};
  for (int i : t) {}
  static_assert(sizeof(t) > 0);
}
template<int... N> void g() { switch (1) { case N: break; } }
template<int... N> void h() { switch (1) { case 1 ... N: break; } }
// Expanded: valid.
template<class... T> constexpr int k(T... t) {
  int n = 0;
  int a[] = {t...};
  for (int i : a) n += i;
  switch (n) { case sizeof...(T): break; }
  return ([](int i = sizeof(t)) { return i; }() + ... + n);
}
static_assert(k(1, 2) == 3 + 4 + 4);
