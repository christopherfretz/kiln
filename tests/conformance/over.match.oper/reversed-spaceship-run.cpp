// [over.match.oper]/8: a reversed `operator<=>` candidate selected for
// `x <=> y` gives `0 <=> (y <=> x)`; the result is a class prvalue
// (codegen's aggregate path) and constant-evaluates the same (libc++
// cmp_nullptr tests, re.submatch compare).
//@ expect: run
//@ stdout: 1 -1 0 -1
extern "C" int printf(const char*, ...);
struct Ord {
  int v;
};
constexpr Ord operator<=>(int z, Ord o) { return Ord{z - o.v}; }
struct P {
  int v;
};
constexpr Ord operator<=>(const P& p, int n) { return Ord{p.v > n ? 1 : p.v < n ? -1 : 0}; }

static_assert((5 <=> P{9}).v == -1);
static_assert((5 <=> P{1}).v == 1);
static_assert((P{1} <=> 5).v == -1);

int main() {
  P a{7}, b{3};
  Ord r1 = 5 <=> b;
  Ord r2 = 5 <=> a;
  printf("%d %d %d %d\n", r1.v, r2.v, (5 <=> P{5}).v, (a <=> 9).v);
}
