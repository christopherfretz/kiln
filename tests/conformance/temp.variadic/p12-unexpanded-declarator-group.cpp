// [temp.variadic]/12: an unexpanded pack in the declaration type of a
// declaration group. As in clang, the error is reported once for a block-
// scope group (the later declarators are parsed but not
// declared, an initializer after them is not consumed) and per declarator
// in a member group. An invalid local declaration in a template is not
// instantiated and ends its compound statement silently: the
// static_assert after `int a[-1]` is not checked in `h<int>`.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:6 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 21:6 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 21:14 error "expected ';' at end of declaration"
//@ diag: 25:6 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 25:9 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 26:7 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 26:10 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 29:9 error "'a' declared as an array with a negative size"
template <class... Ts> void f() {
  Ts a, b;
  Ts c = 1, d = 2;
  int i = 0, *j = &i;
}
template <class... Ts> struct S {
  Ts m, n;
  Ts *p, q;
};
template <class T> int h() {
  int a[-1];
  static_assert(sizeof(T) == 0);
  return 0;
}
int hi = h<int>();
