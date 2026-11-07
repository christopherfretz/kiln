//@ expect: run
//@ stdout: 444 1 8 | 4 1 8 | 1 1 | 3 1 8
// [expr.prim.lambda.capture]/8, [class.mfct.non.static]/2,
// [temp.dep.expr]/3: in a template the call of an overload set with
// static and instance members is resolved at instantiation, where the
// lambda is formed again; `this` is captured only when an instance member
// wins there (clang `TreeTransform` rebuilding the implicit member access
// through `BuildPossibleImplicitMemberExpr`). So a lambda without a
// capture-default may call the static member, and a `[=]` lambda that
// calls only it has no `this` member.
#include <cstdio>
template <class U> struct T1 {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
  template <class P> int call(P p) { return p(3); }
  void run() {
    int a = call([](int s) { return f(s); });
    int b = call([](auto s) { return f(s); });
    int c = call([](int s) { return T1::f(s); });
    auto l = [&](int s) { return f(s); };
    auto m = [&](int s) { return f() + s; };
    std::printf("%d%d%d %d %d | ", a, b, c, (int)sizeof(l), (int)sizeof(m));
  }
};
// Found through a dependent base: `this->A::f` is resolved at
// instantiation too.
struct A {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
};
template <class T> struct Mid : A {};
template <class T> struct D : Mid<T> {
  template <class P> int call(P p) { return p(3); }
  void run() {
    int a = call([](int s) { return A::f(s); });
    auto l = [&](int s) { return A::f(s); };
    auto m = [&](int s) { return A::f() + s; };
    std::printf("%d %d %d | ", a, (int)sizeof(l), (int)sizeof(m));
  }
};
// A member function template of a non-template class.
struct S3 {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
  template <class P> void g(P) {
    auto l = [&](int s) { return f(s); };
    auto m = [&](P s) { return f(s); };
    std::printf("%d %d | ", (int)sizeof(l), (int)sizeof(m));
  }
};
// Nested: the inner lambda's call in a class template.
template <class U> struct T2 {
  int k = 5;
  int f() const { return k; }
  static int f(int x) { return x + 1; }
  void run() {
    auto o = [&] { return [](int s) { return f(s); }(1); };
    auto q = [&] { return [&](int s) { return f(s) + f(); }(1); };
    std::printf("%d %d %d\n", o() + 1, (int)sizeof(o), (int)sizeof(q));
  }
};
int main() {
  T1<int>().run();
  D<int>().run();
  S3().g(0);
  T2<int>().run();
}
