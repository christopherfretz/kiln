// Coverage comparison program (scripts/coverage-compare.py): classes,
// constructors and destructors, templates, lambdas, consteval and
// if consteval, statement expressions, GNU ?:, vector logic, throw.
template <class T> T mx(T a, T b) { return a < b ? b : a; }
template <int N> int fact() { if constexpr (N == 0) return 1; else return N * fact<N - 1>(); }
consteval int ce(int x) { return x > 2 ? x : 2; }
constexpr int cx(int x) { if consteval { return 1; } else { return x ? 2 : 3; } }
struct S {
  int v = 0;
  S() { v = 1; }
  S(int x) : v(x ? x : 1) { if (v > 3) v = 3; }
  S(const S &) = default;
  ~S() { if (v) v = 0; }
  int get() const { return v > 0 ? v : -v; }
};
struct B { virtual ~B() {} virtual int f(int x) { return x > 0; } };
struct D : B { int f(int x) override { return x < 0 ? 1 : 0; } };
struct E : virtual B { E() { } ~E() { } };
struct FT { int x; FT(int a) try : x(a) { if (a) throw 1; } catch (...) { } };
int lam(int n) {
  auto l = [&](int a) { return a > n ? a : n; };
  auto g = [](auto x) { return x ? 1 : 0; };
  int s = l(3) + g(2) + g(0.5);
  auto nested = [=] { return [n] { return n ? 1 : 2; }(); };
  int (*fp)(int) = [](int a) { return a + 1; };
  return s + nested() + fp(1);
}
int stexpr(int x) { return ({ int y = x; if (y) y++; y; }); }
int gnu(int a) { return a ?: 7; }
typedef int v4 __attribute__((vector_size(16)));
v4 vec(v4 a, v4 b) { return a && b; }
v4 vsel(v4 a, v4 b, v4 c) { return a ? b : c; }
int stat(int x) { static int k = x ? 1 : 2; return k; }
int thr(int x) { return x ? x : throw 5; }
int main() {
  int r = mx(1, 2) + (int)mx(1.0, 0.5) + fact<3>() + ce(1) + cx(1);
  S a, b(5);
  S c = b;
  r += a.get() + c.get();
  D d; B &bb = d; r += bb.f(-1);
  E e;
  FT ft(0);
  r += lam(2) + stexpr(1) + gnu(0) + stat(1) + thr(1);
  v4 x = {1, 0, 1, 0}, y = {1, 1, 0, 0};
  r += vec(x, y)[0] + vsel(x, y, y)[1];
  return r & 1 ? 0 : 0;
}
