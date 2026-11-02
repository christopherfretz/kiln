//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 13:10 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 17:11 error "initializer contains unexpanded parameter pack 'xs'"
//@ diag: 22:3 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 26:10 error "expression contains unexpanded parameter pack 'xs'"
// [temp.variadic]/12, [dcl.struct.bind]/1 (P1061): a structured binding
// pack named outside an expansion is diagnosed in the template definition,
// once per full-expression or initializer.
template <class T> int f1(T t) {
  auto [...xs] = t;
  return xs;
}
template <class T> int f2(T t) {
  auto [a, ...xs] = t;
  int k = xs + 1;
  return k;
}
template <class T> void f3(T t) {
  auto [...xs] = t;
  xs;
}
template <class T> int f4(T t) {
  auto [...xs] = t;
  return sizeof(xs);
}
