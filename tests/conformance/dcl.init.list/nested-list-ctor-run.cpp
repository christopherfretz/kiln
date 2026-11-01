// [dcl.init.list]/3.7, [over.match.list]: list-initialization of a
// non-aggregate class whose elements are themselves braced-init-lists
// (each converted to a constructor parameter by [over.ics.list]).
//@ expect: run
//@ stdout: 3 15 4 6 8 8 7 11 7
extern "C" int printf(const char *, ...);
struct P { int a, b; };
struct Q { Q(int x, int y) : v(x * 10 + y) {} int v; };
struct G { G(P p) : v(p.a + p.b) {} G(Q q, int z) : v(q.v + z) {} int v; };
template<class T> struct E { E(P p) : v(p.a * p.b) {} T v; };
struct M { G g; M() : g{{3, 4}} {} };
G make() { return {{5, 6}}; }
template<class T> struct Pair { T a; T b; };
template<class T> struct W { W(Pair<T> p) : v(p.a - p.b) {} T v; };
template<class T> int tf() { W<T> w{{T(9), T(2)}}; return w.v; }
int main() {
  G g{{1, 2}};
  G g2{{1, 2}, 3};
  G g3 = {{2, 2}};
  E<int> e{{2, 3}};
  auto t = G{{4, 4}}.v;
  G *p = new G{{7, 1}};
  M m;
  printf("%d %d %d %d %d %d %d %d %d\n", g.v, g2.v, g3.v, e.v, t, p->v, m.g.v, make().v, tf<int>());
  delete p;
  return 0;
}
