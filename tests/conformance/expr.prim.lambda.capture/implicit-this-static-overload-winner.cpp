//@ expect: run
//@ stdout: 49714
// [expr.prim.lambda.capture]/8, [class.mfct.non.static]/2: an unqualified
// call whose overload set mixes static and instance member functions
// names `this` only when the instance member wins, so a lambda without a
// capture-default may call the static one (clang's
// BuildPossibleImplicitMemberExpr).
#include <cstdio>
struct S {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
  template <class T> static int t(T x) { return x * 2; }
  template <class T> int t(T x, T y) const { return x + y + k; }
  template <class P> int call(P p) { return p(3); }
  int run() {
    int a = call([](int s) { return f(s); });
    int b = call([&](int s) { return f() + f(s); });
    int c = call([](int s) { return t<int>(s); });
    int d = call([=, this](int s) { return t<int>(s, s); });
    int e = call([](int s) { return [](int u) { return f(u); }(s); });
    return a * 10000 + b * 1000 + c * 100 + d * 10 + e;
  }
};
int main() {
  S s;
  std::printf("%d\n", s.run());
}
