// [expr.prim.lambda.capture]/8, [basic.def.odr]/5: in a generic lambda a
// use of a constexpr local in a full-expression that depends on the
// lambda's template parameter is decided when the call operator is
// instantiated; an odr-use then needs a capture that a lambda without a
// capture-default does not have: `err_lambda_impcap` in the
// instantiation, its fix-it notes at the `[`. Reading the value needs none.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:40 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 36:12 note "in instantiation of function template specialization 'f()::(lambda)::operator()<int>' requested here"
//@ diag: 28:15 note "'b' declared here"
//@ diag: 32:13 note "lambda expression begins here"
//@ diag: 32:13 note "capture 'b' by value"
//@ diag: 32:13 note "capture 'b' by reference"
//@ diag: 32:14 note "default capture by value"
//@ diag: 32:14 note "default capture by reference"
//@ diag: 33:41 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 36:20 note "in instantiation of function template specialization 'f()::(lambda)::operator()<int>' requested here"
//@ diag: 28:15 note "'b' declared here"
//@ diag: 33:13 note "lambda expression begins here"
//@ diag: 33:13 note "capture 'b' by value"
//@ diag: 33:13 note "capture 'b' by reference"
struct A { int x, y; };
template <class T> const A &id(const A &a, T) { return a; }
template <class T> int val(int v, T) { return v; }
int f() {
  constexpr A b{1, 2};
  constexpr int k = 3;
  // An odr-use in a full-expression that depends on the generic
  // parameter: decided, and diagnosed, in the instantiation.
  auto l1 = [](auto t) { auto &&r = id(b, t); return r.x; };
  auto l2 = [k = 0](auto t) { return id(b, t).y + k; };
  // No odr-use: the values are read.
  auto ok = [](auto t) { return val(b.x, t) + val(k, t); };
  return l1(0) + l2(0) + ok(0);
}
