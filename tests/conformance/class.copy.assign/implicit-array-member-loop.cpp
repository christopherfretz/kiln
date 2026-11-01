// [class.copy.assign]/12.2: an implicitly-defined copy/move assignment operator assigns an array
// member element-wise with each element's own (non-trivial) assignment operator, moving the
// elements of an xvalue source; usable in constant evaluation and at run time
//@ expect: run
//@ strict-diags
//@ stdout: 7 1 1 9
extern "C" int printf(const char*, ...);
struct P {
  int x = 0, moved = 0, copied = 0;
  constexpr P& operator=(const P& o) { x = o.x; copied++; return *this; }
  constexpr P& operator=(P&& o) { x = o.x; moved++; return *this; }
};
struct S { P p[2][3]; int k[2]; };
struct One { P p[1]; };
template <class T, unsigned N> struct arr { T elems[N]; };
constexpr int f() {
  S a{}, b{};
  b.p[1][2].x = 7; b.k[1] = 4;
  a = b;
  int r = a.p[1][2].x * 100 + a.p[0][0].copied * 10 + a.k[1];
  a = static_cast<S&&>(b);
  return r * 10 + a.p[1][1].moved;
}
static_assert(f() == 7141);
constexpr bool g() {
  arr<P, 1> a{}, b{};
  b.elems[0].x = 3;
  a = b;
  One o{}, q{};
  o = static_cast<One&&>(q);
  return a.elems[0].x == 3 && a.elems[0].copied == 1 && o.p[0].moved == 1;
}
static_assert(g());
int main() {
  S a{}, b{};
  b.p[1][2].x = 7;
  a = b;
  a = static_cast<S&&>(b);
  arr<P, 2> c{}, d{};
  d.elems[1].x = 9;
  c = d;
  printf("%d %d %d %d\n", a.p[1][2].x, a.p[0][1].copied, a.p[1][0].moved, c.elems[1].x);
}
