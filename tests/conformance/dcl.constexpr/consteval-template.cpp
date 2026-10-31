// [temp.res.general]/6, [expr.const]/16: a dependent immediate invocation
// is checked when the template is instantiated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:45 error "call to consteval function 'sq' is not a constant expression"
//@ diag: 20:11 note "in instantiation of function template specialization 'direct<int>' requested here"
//@ diag: 14:48 note "function parameter 't' with unknown value cannot be used in a constant expression"
//@ diag: 14:33 note "declared here"
//@ diag: 14:45 error "call to consteval function 'sq' is not a constant expression"
//@ diag: 21:11 note "in instantiation of function template specialization 'direct<long>' requested here"
//@ diag: 14:48 note "function parameter 't' with unknown value cannot be used in a constant expression"
//@ diag: 14:33 note "declared here"
consteval int sq(int x) { return x * x; }
template <class T> int direct(T t) { return sq(t); }
template <class T> int fixed(T) { return sq(3); }
template <int N> int nttp() { return sq(N); }
int run(int n) {
  int a = fixed(n);
  int b = nttp<5>();
  int c = direct(n);
  int d = direct(2L);
  return a + b + c + d;
}
