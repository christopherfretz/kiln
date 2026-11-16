// [expr.prim.lambda.capture]/8, [basic.def.odr]/5, [stmt.expand]/5: the
// range of an iterating or destructuring expansion statement initializes
// a reference (`auto&& __range = b`, `auto&& [__u...] = b`), so naming a
// local of the enclosing function there odr-uses it even when it is
// `constexpr`, and a lambda without a capture-default cannot use it; an
// element bound to a reference odr-uses its name too. In a generic lambda
// the error comes with the instantiation, as clang reports any odr-use a
// generic lambda's definition cannot decide. Found running GCC's C++
// testsuite (GCC 16.1 rejects each, "not captured").
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 83:28 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 64:17 note "'a' declared here"
//@ diag: 81:13 note "lambda expression begins here"
//@ diag: 81:14 note "capture 'a' by value"
//@ diag: 81:14 note "capture 'a' by reference"
//@ diag: 81:14 note "default capture by value"
//@ diag: 81:14 note "default capture by reference"
//@ diag: 90:30 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 92:6 note "in instantiation of function template specialization 'f()::("
//@ diag: 63:15 note "'b' declared here"
//@ diag: 88:12 note "lambda expression begins here"
//@ diag: 88:12 note "capture 'b' by value"
//@ diag: 88:12 note "capture 'b' by reference"
//@ diag: 88:13 note "default capture by value"
//@ diag: 88:13 note "default capture by reference"
//@ diag: 98:28 error "variable 'v' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 95:9 note "'v' declared here"
//@ diag: 96:13 note "lambda expression begins here"
//@ diag: 96:14 note "capture 'v' by value"
//@ diag: 96:14 note "capture 'v' by reference"
//@ diag: 96:14 note "default capture by value"
//@ diag: 96:14 note "default capture by reference"
//@ diag: 67:28 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 101:12 note "in instantiation of function template specialization 'f()::("
//@ diag: 63:15 note "'b' declared here"
//@ diag: 65:13 note "lambda expression begins here"
//@ diag: 65:13 note "capture 'b' by value"
//@ diag: 65:13 note "capture 'b' by reference"
//@ diag: 65:14 note "default capture by value"
//@ diag: 65:14 note "default capture by reference"
//@ diag: 72:28 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 101:20 note "in instantiation of function template specialization 'f()::("
//@ diag: 64:17 note "'a' declared here"
//@ diag: 70:13 note "lambda expression begins here"
//@ diag: 70:13 note "capture 'a' by value"
//@ diag: 70:13 note "capture 'a' by reference"
//@ diag: 70:14 note "default capture by value"
//@ diag: 70:14 note "default capture by reference"
//@ diag: 77:33 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 101:28 note "in instantiation of function template specialization 'f()::("
//@ diag: 63:15 note "'b' declared here"
//@ diag: 75:13 note "lambda expression begins here"
//@ diag: 75:13 note "capture 'b' by value"
//@ diag: 75:13 note "capture 'b' by reference"
//@ diag: 75:14 note "default capture by value"
//@ diag: 75:14 note "default capture by reference"
struct B { int x = 1, y = 2; };
struct P { int v[2]; int n; };
int f() {
  constexpr B b;
  constexpr int a[] = {3, 4};
  auto l1 = [](auto) {
    int s = 0;
    template for (auto e : b) { s += e; }
    return s;
  };
  auto l2 = [](auto) {
    int s = 0;
    template for (auto e : a) { s += e; }
    return s;
  };
  auto l3 = [](auto) {
    int s = 0;
    template for (const B& e : {b}) { s += e.x; }
    return s;
  };
  // Not a generic lambda, and a for-range-declaration without constexpr.
  auto l4 = [] {
    int s = 0;
    template for (auto e : a) { s += e; }
    return s;
  };
  // The inner lambda has no capture-default, whatever the outer one does.
  auto l5 = [=] {
    return [](auto) {
      int s = 0;
      template for (auto e : b) { s += e; }
      return s;
    }(0);
  };
  // A structured binding is captured when named: a capture error at once.
  auto [v, n] = P{{5, 6}, 2};
  auto l6 = [](auto) {
    int s = 0;
    template for (auto e : v) { s += e; }
    return s;
  };
  return l1(0) + l2(0) + l3(0) + l4() + l5() + l6(0) + n;
}
