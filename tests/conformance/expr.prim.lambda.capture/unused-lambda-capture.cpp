// [expr.prim.lambda.capture]: -Wunused-lambda-capture (clang's
// DiagnoseUnusedLambdaCapture). An explicit capture the body never
// odr-uses is "not used"; one named only in an unevaluated operand or as
// a constant is "not required to be captured for this use". Copies with
// side effects (volatile, non-trivial copy or destructor, an init-capture
// with a side-effecting initializer) and generic lambdas' non-odr uses are
// exempt. Instantiated lambdas warn once per instantiation.
//@ verify
//@ args: -Wunused-lambda-capture

struct NT { NT(); NT(const NT&); };
struct S {
  int m;
  void f() {
    auto a = [this] {}; // expected-warning {{lambda capture 'this' is not used}}
    auto b = [this] { return m; };
    auto c = [*this] {}; // expected-warning {{lambda capture 'this' is not used}}
    auto d = [this] { return sizeof(*this); }; // expected-warning {{lambda capture 'this' is not used}}
  }
};
void g(int p) {
  int x = 0, y = 0;
  const int k = 5;
  volatile int v = 0;
  NT nt;
  int arr[3];
  auto l1 = [x] {}; // expected-warning {{lambda capture 'x' is not used}}
  auto l2 = [x, y] { return y; }; // expected-warning {{lambda capture 'x' is not used}}
  auto l3 = [x, y] { return x; }; // expected-warning {{lambda capture 'y' is not used}}
  auto l4 = [&x, y] {}; // expected-warning {{lambda capture 'x' is not used}} expected-warning {{lambda capture 'y' is not used}}
  auto l5 = [=, &x] {}; // expected-warning {{lambda capture 'x' is not used}}
  auto l6 = [k] { return k; }; // expected-warning {{lambda capture 'k' is not required to be captured for this use}}
  auto l7 = [k] { return &k; };
  auto l8 = [v] {};
  auto l9 = [nt] {};
  auto l10 = [i = x + 1] {}; // expected-warning {{lambda capture 'i' is not used}}
  auto l11 = [i = g] {}; // expected-warning {{lambda capture 'i' is not used}}
  auto l12 = [arr] {}; // expected-warning {{lambda capture 'arr' is not used}}
  auto l13 = [x] { return sizeof(x); }; // expected-warning {{lambda capture 'x' is not required to be captured for this use}}
  auto l14 = [x](auto) { return x; };
  auto l15 = [i = 1](auto) { return i; };
  auto l16 = [x] { return [x] { return x; }; };
  auto l17 = [x, y, p] { return y; }; // expected-warning {{lambda capture 'x' is not used}} expected-warning {{lambda capture 'p' is not used}}
  auto l19 = [p, x = p] { return x; }; // expected-warning {{lambda capture 'p' is not used}}
  auto l20 = [_ = x] {};
}

template <class T> void t(T q) {
  auto l = [q] {}; // expected-warning {{lambda capture 'q' is not used}}
  int z = 0;
  auto m = [z] {}; // expected-warning {{lambda capture 'z' is not used}}
}
template void t(int); // expected-note {{in instantiation of function template specialization 't<int>' requested here}}

template <class... Ts> void pk(Ts... ts) {
  auto l = [ts...] {}; // expected-warning 2 {{lambda capture 'ts' is not used}}
}
template void pk(int, long); // expected-note {{in instantiation of function template specialization 'pk<int, long>' requested here}}

#define CAP [x]
void mac(int x) {
  auto l = CAP {}; // expected-warning {{lambda capture 'x' is not used}}
}
