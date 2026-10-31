// [temp.dep.expr]/3, [over.match.viable]: a braced-init-list argument of a
// dependent call that cannot initialize its parameter makes the call
// ill-formed at instantiation. WP-128.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:10 error "no matching function for call to 'sum'"
//@ diag: 15:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 11:5 note "cannot convert initializer list argument to 'P'"

struct P { int a, b; };
int sum(int, P);
template <class T> int f(T t) {
  return sum(t, {1, 2, 3});
}
int x = f(1);
