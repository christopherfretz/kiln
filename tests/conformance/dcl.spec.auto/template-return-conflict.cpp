// [dcl.spec.auto.general]/9 at instantiation: non-discarded returns that
// deduce different types are a genuine conflict.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:3 error "'auto' in return type deduced as 'int' here but deduced as 'void' in earlier return statement"
//@ diag: 12:10 note "in instantiation of function template specialization 'f<int>' requested here"
template <class T> auto f(bool b) {
  if (b)
    return;
  return T();
}
int x = (f<int>(true), 0);
