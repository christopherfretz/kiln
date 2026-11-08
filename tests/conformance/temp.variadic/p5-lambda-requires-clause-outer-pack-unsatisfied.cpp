// [temp.variadic]/5, [temp.constr.decl]/3: the closure a pack expansion
// instantiates for each element of an enclosing pack named in the lambda's
// trailing requires-clause checks the constraint with that element; for
// `long` it is not satisfied and the call has no viable candidate. WP-776.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:21 error "no matching function for call to object of type '(lambda at "
//@ diag: 16:9 note "in instantiation of function template specialization 'f<int, long>' requested here"
//@ diag: 14:21 note "candidate template ignored: constraints not satisfied [with U = int]"
//@ diag: 14:45 note "because 'Same<int, long>' evaluated to false"
//@ diag: 12:44 note "because '__is_same(int, long)' evaluated to false"
template <class T, class U> concept Same = __is_same(T, U);
template <class... Ts> int f() {
  return (0 + ... + []<class U>(U) requires Same<U, Ts> { return 1; }(0));
}
int x = f<int, long>();
