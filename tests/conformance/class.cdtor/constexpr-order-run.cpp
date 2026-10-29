// [class.base.init]/13, [class.dtor]/13, [class.temporary]/6: the
// construction and destruction order of class objects in constant
// evaluation, observed at run time through constexpr variables (no class
// code runs at run time).
//@ expect: run
//@ stdout: 10 3 4 -4 -3 -10
//@ stdout: 7 8 15 -8 -7
extern "C" int printf(const char *, ...);
struct Log { int v[8] = {}; int n = 0; constexpr void add(int x) { v[n++] = x; } };
struct T {
  Log *l; int id;
  constexpr T(Log *l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
struct Base { Log *l; constexpr Base(Log *l) : l(l) { l->add(10); } constexpr ~Base() { l->add(-10); } };
struct Der : Base { T t; constexpr Der(Log *l) : Base(l), t(l, 3) { l->add(4); } constexpr ~Der() { l->add(-4); } };
constexpr Log bases() { Log l; { Der d(&l); } return l; }
constexpr Log extended() { Log l; { const T &r = T(&l, 7); T &&s = T(&l, 8); l.add(r.id + s.id); } return l; }
constexpr Log a = bases();
constexpr Log b = extended();
void print(const Log &l) {
  for (int i = 0; i < l.n; ++i) printf(i ? " %d" : "%d", l.v[i]);
  printf("\n");
}
int main() {
  print(a);
  print(b);
}
