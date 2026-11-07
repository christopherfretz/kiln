//@ verify
// [expr.prim.lambda.capture]/8: an instance member that wins the call of
// a mixed static/instance overload set only at instantiation still needs
// `this`, which a lambda without a capture-default cannot capture: in a
// class template, and in a generic lambda's call operator.
template <class U> struct T {
  int k = 5;
  int f(double) const { return k; }
  static int f(int x) { return x + 1; }
  int run() {
    return [](double s) { // expected-note {{explicitly capture 'this'}} expected-note {{while substituting into a lambda expression here}}
      return f(s); // expected-error {{'this' cannot be implicitly captured in this context}}
    }(1.0);
  }
};
struct S {
  int k = 5;
  int f(double) const { return k; }
  static int f(int x) { return x + 1; }
  int run() {
    return [](auto s) { // expected-note {{explicitly capture 'this'}}
      return f(s); // expected-error {{'this' cannot be implicitly captured in this context}}
    }(1.0); // expected-note {{in instantiation of function template specialization}}
  }
  int ok() {
    return [](auto s) { return f(s); }(1);
  }
};
int a = T<int>().run(); // expected-note {{in instantiation of member function 'T<int>::run' requested here}}
int b = S().run() + S().ok();
