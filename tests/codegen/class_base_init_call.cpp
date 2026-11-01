// A base initialized from a prvalue of its own type is not elided
// (CWG2403): the call result is materialized and the base is copy- or
// move-constructed from it (conversion functions are not candidates
// there, clang `RequireActualConstructor`); trivial copies use the
// base's data size. An aggregate's base element calls the base variant.
struct T {
  int a;
  char c;
  T(int x) : a(x), c(1) {}
  T(const T &) = default;
};
T tmake(int);
struct TD : T {
  char d;
  TD() : T(tmake(3)), d(7) {}
};
void tuse() { TD g; }
struct B {
  int a;
  char c;
  B(int x) : a(x), c(1) {}
  B(const B &o) : a(o.a), c(o.c) {}
  B(B &&o) : a(o.a + 1), c(o.c) {}
  ~B() {}
};
B make(int);
struct V { int v; V(int x) : v(x) {} };
V mkv();
struct D : B, virtual V {
  char d;
  D() : B(make(3)), V(mkv()), d(7) {}
  D(int) : B{make(4)}, V{mkv()}, d(8) {}
  D(char) : D(mkD()) {}
  static D mkD();
};
struct Agg : B { char e; };
struct Conv { operator B() const; };
struct E : B {
  E() : B(Conv()) {}
  E(int) : B{Conv()} {}
};
int use() {
  D a, b(1), c('c');
  Agg g{make(5), 6};
  E e, e2(1);
  return a.d + g.e;
}
