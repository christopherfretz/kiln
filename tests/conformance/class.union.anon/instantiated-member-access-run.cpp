// [class.union.anon]/1, [expr.ref]/2, /7, [temp.inst]: instantiated
// accesses to anonymous union members through a pointer, a derived
// object and a nested anonymous struct read and write the right
// objects at run time and in constant evaluation. Reduced from LLVM 19
// (`TypePropertyCache::ensure`).
//@ expect: run
//@ stdout: 2 3 1 5 3
extern "C" int printf(const char *, ...);
struct T { union { unsigned a : 2; int other; }; };
struct N { union { struct { unsigned a : 2; int y; }; long other; }; };
struct D : T { int z; };
template <class P> constexpr void copy(T *t, T *ct) { t->a = ct->a; }
template <class P> unsigned read(D *d) { return d->a; }
template <class P> void bump(N &n) { n.a += 1; n.y = n.y * 5; }
template <class P> constexpr unsigned cx() {
  T t{}, u{};
  u.a = 3;
  copy<P>(&t, &u);
  return t.a;
}
int main() {
  T x{}, y{};
  y.a = 2;
  copy<int>(&x, &y);
  D d{};
  d.a = 3;
  N n{};
  n.y = 1;
  bump<int>(n);
  constexpr unsigned c = cx<int>();
  printf("%u %u %u %d %u\n", x.a, read<int>(&d), n.a, n.y, c);
}
