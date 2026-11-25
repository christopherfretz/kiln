// [stmt.ranged]/1, [dcl.fct]/5: a parameter declared with array type is
// a pointer, so it is no range. In the definition clang names the array
// parameter (err_range_on_array_parameter); in an instantiation, whose
// parameter keeps its declared type too, it gives the plain invalid-range
// error, as clang's rebuild does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:16 error "cannot build range expression with array function parameter 'a' since parameter with array type 'int[3]' is treated as pointer type 'int *'"
//@ diag: 13:11 note "declared here"
//@ diag: 21:15 error "invalid range expression of type 'int *'; no viable 'begin' function available"
//@ diag: 25:24 note "in instantiation of function template specialization 'g<int>' requested here"

int f(int a[3]) {
  int s = 0;
  for (int x : a) s += x;
  return s;
}

template <class T> int g(T a[3]) {
  int s = 0;
  for (auto x : a) s += x;
  return s;
}

int h(int *p) { return g<int>(p); }
