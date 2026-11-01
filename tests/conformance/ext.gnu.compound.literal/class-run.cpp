// Class compound literals (WP-191) are temporaries: members read through
// them, they initialize and bind, and destructors run at the end of the
// full-expression; also in constant evaluation and instantiations.
//@ expect: run
//@ stdout: 3 7 6 7 1 17 2 12
extern "C" int printf(const char *, ...);
union U { int a; float f; };
struct P { int a, b; };
int dtors;
struct D { int a; ~D() { ++dtors; } };
struct Q { Q(int x) : v(x * 2) {} int v; };
struct E { E(int x, int y) : v(x + y) {} ~E() { ++dtors; } int v; };
constexpr int k() { return (P){8, 9}.b + (U){3}.a; }
static_assert(k() == 12);
template<class T> int t(int x) { return (T){x, 2}.b + (T){x, 5}.a; }
int main() {
  int r = (U){1}.a + (P){1, 2}.b;
  P p = (P){3, 4};
  const P &ref = (P){5, 6};
  int d = (D){7}.a;
  int before = dtors;
  int q = (Q){7}.v + (E){1, 2}.v;
  int s = t<P>(10);
  printf("%d %d %d %d %d %d %d %d\n", r, p.a + p.b, ref.b, d, before, q, dtors, s);
  return r == 3 && p.a == 3 && ref.b == 6 && d == 7 && before == 1 && q == 17 && dtors == 2 && s == 12 ? 0 : 1;
}
