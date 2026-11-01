// [expr.const]/16, [stmt.if]/4: immediate invocations are replaced by
// their values (scalars and aggregates); `if consteval` picks the
// run-time branch when not constant-evaluated.
//@ expect: run
//@ stdout: 9 3 4 5 6 70 12 1 7 2
extern "C" int printf(const char*, ...);

consteval int sq(int x) { return x * x; }
struct P { int a; int b; };
struct Big { int v[20]; };
consteval P mk(int x) { return {x, x + 1}; }
consteval Big big() { Big b{}; b.v[3] = 70; return b; }
struct C {
  int v;
  consteval C(int x) : v(x * 2) {}
};
constexpr int where() {
  if consteval {
    return 7;
  } else {
    return 1;
  }
}
constexpr int k = where();

int main() {
  P p = mk(3);
  P q;
  q = mk(5);
  Big b = big();
  C c(6);
  auto inc = [](int y) consteval { return y + 1; };
  printf("%d %d %d %d %d %d %d %d %d %d\n", sq(3), p.a, p.b, q.a, q.b, b.v[3], c.v, where(), k,
         inc(1));
}
