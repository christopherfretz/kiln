//@ expect: run
//@ stdout: 4 4 1 8 8 | 2 8 1 | 20 1 8
// [expr.prim.lambda.capture]/7-8: a generic lambda's closure is complete
// before its call operator is instantiated, so a call of an overload set
// with static and instance members that depends on a generic parameter
// captures `this` when the lambda is formed if every enclosing lambda can
// (has a capture-default), and otherwise captures nothing: the static
// member may then be called, an instance winner is an error (clang
// `tryImplicitlyCaptureThisIfImplicitMemberFunctionAccessWithDependentArgs`).
// A non-dependent call captures only for an instance winner.
#include <cstdio>
struct S {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
  template <class P> int call(P p) { return p(3); }
  void run() {
    int b = call([](auto s) { return f(s); });
    int c = call([](int s) { return S::f(s); });
    auto l = [&](int s) { return f(s); };
    auto m = [&](auto s) { return f(s); };
    auto n = [&](auto) { return f(); };
    std::printf("%d %d %d %d %d | ", b, c, (int)sizeof(l), (int)sizeof(m),
                (int)sizeof(n));
  }
};
// Nested generic lambdas: the outer one is the innermost lambda whose
// closure is formed outside a template, so it captures `this`.
struct N {
  int k = 5;
  int f(double) const { return k; }
  static int f(int x) { return x + 1; }
  void run() {
    auto o = [&](auto a) { return [&](auto s) { return f(s); }(a); };
    auto p = [](auto a) { return [&](auto s) { return f(s); }(a); };
    std::printf("%d %d %d | ", o(1), (int)sizeof(o), (int)sizeof(p));
    (void)p(1);
  }
};
// An explicit object member function is an instance member too
// ([expr.prim.id.general]/2).
struct E {
  int k = 7;
  int f(this const E& self, double) { return self.k * 2 + 6; }
  static int f(int x) { return x + 1; }
  void run() {
    auto l = [&](int s) { return f(s); };
    auto m = [&](auto s) { return f(s); };
    std::printf("%d %d %d\n", m(1.0), (int)sizeof(l), (int)sizeof(m));
  }
};
int main() {
  S().run();
  N().run();
  E().run();
}
