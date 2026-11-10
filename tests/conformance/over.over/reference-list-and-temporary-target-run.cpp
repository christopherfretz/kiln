// [over.over]/1, [dcl.init.list]/3.10, [dcl.init.ref]/5.4.2: a reference
// to function list-initialized from an overload set binds to the function
// the target selects, as `FR r = g` does; a reference to const pointer
// to function initialized from one binds to a temporary pointer to the
// selected function. Both work for a plain overload set and for a
// template-id the target completes (WP-1170).
//@ expect: run
//@ stdout: 1 2 3 4 5 6
extern "C" int printf(const char *, ...);
int g(int) { return 1; }
int g(long) { return 0; }
template <class T, class U> int f(U *, T) { return 4; }
template <class T> int f(long, T) { return 0; }
using GR = int (&)(int);
using GP = int (*)(int);
using FR = int (&)(int *, int);
using FP = int (*)(int *, int);
int main() {
  GR a{g};
  GR b(g);
  const GP &c = g;
  FR d{f<int>};
  FR e(f<int>);
  const FP &h = f<int>;
  printf("%d %d %d %d %d %d\n", a(0), b(0) + 1, c(0) + 2, d(nullptr, 0),
         e(nullptr, 0) + 1, h(nullptr, 0) + 2);
}
