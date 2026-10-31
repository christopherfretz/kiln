// [expr.prim.lambda.closure]/3-4, /9: calls to a generic lambda deduce
// the call operator's template arguments (conflicts and unsatisfied
// constraints reject it), and the conversion function template's
// specialization has the call operator's deduced return type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "no matching function for call to object of type '(lambda at "
//@ diag: 23:13 note "candidate template ignored: constraints not satisfied [with x:auto = double]"
//@ diag: 23:16 note "because 'double' does not satisfy 'Small'"
//@ diag: 21:36 note "because 'sizeof(double) <= 4' (8 <= 4) evaluated to false"
//@ diag: 26:3 error "no matching function for call to object of type '(lambda at "
//@ diag: 25:13 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double')"
//@ diag: 28:10 error "no viable conversion from '(lambda at "
//@ diag: 27:12 note "candidate function [with x:auto = int]"
//@ diag: 30:3 error "no matching function for call to object of type '(lambda at "
//@ diag: 29:12 note "candidate template ignored: constraints not satisfied [with T = long long]"
//@ diag: 29:33 note "because 'long long' does not satisfy 'Small'"
//@ diag: 21:36 note "because 'sizeof(long long) <= 4' (8 <= 4) evaluated to false"
//@ diag: 31:26 error "member reference base type 'int' is not a structure or union"
//@ diag: 32:4 note "in instantiation of function template specialization 'f()::(anonymous class)::operator()<int>' requested here"
template <class T> concept Small = sizeof(T) <= 4;
void f() {
  auto cn = [](Small auto x) { return x + 1; };
  cn(1.0);
  auto tp = []<class T>(T a, T b) { return a; };
  tp(1, 2.0);
  auto g = [](auto x) { return x; };
  void (*p)(int) = g;
  auto h = []<class T> requires Small<T> (T x) { return x; };
  h(1LL);
  auto u = [](auto x) { x.foo(); };
  u(1);
}
