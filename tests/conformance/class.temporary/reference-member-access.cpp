// [class.temporary]/6.7: a class member access extends (or, returned,
// dangles) the temporary it is applied to only when it designates a
// non-static data member of non-reference type. A reference member denotes
// what it was bound to, so returning `make().first` is not "returning
// reference to local temporary object" (libc++ __node_handle:182
// `__get_value().__ref().first`), and `int &r = make().first;` does not
// extend `make()`'s temporary: it is destroyed at the end of the
// full-expression ([class.temporary]/4).
//@ expect: run
//@ strict-diags
//@ stdout: dtor after dtor dtor dtor 1 7
extern "C" int printf(const char *, ...);
struct P {
  int &first;
  int &&second;
  ~P() { printf("dtor "); }
};
int g = 7;
P make() { return P{g, static_cast<int &&>(g)}; }
int &f1() { return make().first; }
int &&f2() { return static_cast<int &&>(make().second); }
int &f3() { P p = make(); return p.first; }
int main() {
  int &r = make().first;
  printf("after ");
  int &&x = f2();
  printf("%d %d\n", &r == &f1() && &x == &f3(), r);
}
