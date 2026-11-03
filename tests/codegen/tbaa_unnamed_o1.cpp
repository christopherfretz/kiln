//@ args: -O1
// Unnamed classes at -O1: the struct-path TBAA node of an unnamed class
// is its `_ZTS` name with the per-scope `<unnamed-type-name>`
// discriminator (`Ut_`, `Ut0_`, `Ut1_` ...; Itanium C++ ABI §5.1.5.6,
// [class.pre]/1), so two unnamed classes of one class (an anonymous union
// counted between them) or of one function get distinct nodes and
// symbols. `optnone` keeps the bodies.
#define ON __attribute__((optnone))
struct A {
  struct { int x; float y; } a;
  union { int u; };
  struct { long z; int w; } b;
  ON int f() { return a.x + b.w + u; }
};
ON int g(A* p) { p->a.y = 1; p->b.z = 2; return p->f(); }
inline int h() {
  struct { int p; ON int f() { return p; } } s{1};
  struct { short q; ON int f() { return q; } } t{2};
  return s.f() + t.f();
}
ON int use() { return h(); }
