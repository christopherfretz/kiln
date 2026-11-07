//@ verify
// [expr.prim.lambda.capture]/8: when the instance member of a mixed
// static/instance overload set wins, the implicit `this` must be
// capturable.
struct S {
  int f() const { return 1; }
  static int f(int x) { return x; }
  int g() {
    return [](int s) { // expected-note {{explicitly capture 'this'}}
      return f() + s; // expected-error {{'this' cannot be implicitly captured in this context}}
    }(1);
  }
};
