// [temp.res.general]/6, [temp.inst], [expr.type.conv] with the GNU/C99
// compound literal extension: a compound literal `(T){...}` of a
// non-dependent type whose elements are only instantiation-dependent
// (locals of a template) is analyzed in the pattern and initialized again
// from the substituted list at instantiation: a class's list construction
// ([dcl.init.list]/3.7), a single element of the class
// ([dcl.init.list]/3.2), an initializer_list ([dcl.init.list]/5), an
// aggregate with designators ([dcl.init.aggr]/3.1) and an array. WP-301.
//@ args: -Wno-c99-extensions
//@ expect: compile-ok
#include <initializer_list>

struct P { int x, y; };
struct C {
  int v;
  constexpr C(int a, int b) : v(a * 10 + b) {}
  constexpr C(const C& o) : v(o.v + 1) {}
};
struct L {
  int n;
  constexpr L(std::initializer_list<int> l) : n(int(l.size())) {}
};

template <int K> constexpr int f() {
  int a = K;
  C c0(a, 1);
  C c = (C){a, 2};              // list construction: 32
  C cc = (C){c0};               // copy from the one element: 31 + 1
  L l = (L){a, a, a};           // 3
  P p = (P){.x = a, .y = a + 1};
  P q = (P){a};
  int e = ((int[]){a, a + 5, a})[1];
  return c.v + cc.v + l.n + p.x + p.y + q.x + q.y + e;
}
static_assert(f<3>() == 32 + 32 + 3 + 3 + 4 + 3 + 0 + 8);

int main() {}
