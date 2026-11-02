// [temp.res.general]/6, [temp.inst], [expr.new]/23: a new-expression of a
// non-dependent allocated type whose operands are only
// instantiation-dependent (a local of the template inside a nested
// braced-init-list) is analyzed in the pattern; its instantiation is
// formed again from the written placement arguments, bound and
// initializer (parentheses or braces kept), so the allocation function
// is looked up and the elements initialized anew (clang
// `TransformCXXNewExpr`). WP-301.
//@ expect: run
//@ stdout: 94 102 7
#include <cstdio>
#include <new>

struct S {
  int v;
  constexpr S(int a, int b = 0) : v(a * 10 + b) {}
  constexpr S() : v(5) {}
};
struct Q { int x, y; };

template <int K> constexpr int g() {
  int a = K;
  S* s1 = new S[2]{{a}, {a, 1}};          // 20 21
  Q* q1 = new Q{Q{a, 1}, };               // 2 1
  Q* q2 = new Q[2]{{a, 1}, {a}};          // 2 1, 2 0
  Q* q3 = new Q[]{{a, 1}, {a}, {}};       // bound from the list
  S* s2 = new S[3]({a, 1}, {a});          // P0960: 21 20 5
  int r = s1[0].v + s1[1].v + q1->x + q1->y + q2[0].x + q2[1].x + q2[1].y + q3[2].x
        + s2[0].v + s2[1].v + s2[2].v;
  delete[] s1;
  delete q1;
  delete[] q2;
  delete[] q3;
  delete[] s2;
  return r;
}
static_assert(g<2>() == 20 + 21 + 2 + 1 + 2 + 2 + 0 + 0 + 21 + 20 + 5);

template <int K> int h() {
  int a = K;
  alignas(S) unsigned char buf[2 * sizeof(S)];
  S* p = ::new (buf) S[2]{{a}, {a, 2}};   // placement, analyzed
  return p[0].v + p[1].v;
}

template <int K> int local() {
  struct Lc { int v = K; };
  Lc* a = new Lc;
  Lc* b = new Lc();
  Lc* c = new Lc[2];
  Lc* d = new Lc{};
  int r = a->v + b->v + c[1].v + d->v;
  delete a;
  delete b;
  delete[] c;
  delete d;
  return r;
}

int main() {
  std::printf("%d %d %d\n", g<2>(), h<5>(), local<1>() + 3);
}
