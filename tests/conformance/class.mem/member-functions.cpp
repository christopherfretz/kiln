// [class.mfct.non.static]: member functions see the members through an
// implicit this->; default member initializers; const and ref-qualified
// member functions; mutable members are modifiable through a const object.
//@ expect: run
//@ stdout: 9 18 7 10 1 2 2
//@ xfail(kiln): needs WP-065 parser classes
extern "C" int printf(const char*, ...);
struct S {
  int x = 7;
  mutable int hits = 0;
  int get() const { ++hits; return x + 2; }
  int twice() const { return get() * 2; }
  S *self() { return this; }
  int lv() & { return 1; }
  int clv() const & { return 2; }
  int set(int v = 10) { x = v; return this->x; }
};
int main() {
  S s;
  const S &cs = s;
  int a = cs.get(), b = s.twice(), c = s.self()->x;
  int d = s.set();
  printf("%d %d %d %d %d %d %d\n", a, b, c, d, s.lv(), cs.clv(), s.hits);
}
