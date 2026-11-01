//@ expect: ast
// [dcl.constexpr]/2: consteval functions, members, constructors,
// conversion functions, lambdas and function templates.
consteval int sq(int x) { return x * x; }
struct S {
  int v;
  consteval S(int x) : v(x) {}
  consteval int get() const { return v; }
  consteval operator long() const { return v; }
  static consteval int twice(int x) { return 2 * x; }
};
struct V { virtual consteval int vf() const { return 1; } };
template <class T> consteval T ident(T t) { return t; }
auto lam = [](int y) consteval { return y + 1; };
consteval int forward_declared();
consteval int forward_declared() { return 3; }
