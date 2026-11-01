// [temp.res.general]/6, [basic.def.odr]/7 (WP-137): a non-dependent
// expression of a function template is shared by its instantiations, which
// still odr-use its functions: `f2<int>` and the members of `X<int>`
// (constructors, destructor, conversion, class-specific `operator new` /
// `operator delete`, operators) are instantiated and emitted.
//@ expect: run
//@ stdout: new
//@ stdout: ~X
//@ stdout: del
//@ stdout: ~X
//@ stdout: ~X
//@ stdout: ~X
//@ stdout: 222
extern "C" int printf(const char *, ...);
extern "C" void *malloc(unsigned long); extern "C" void free(void *);
template <class T> int f2() { return sizeof(T); }
inline int h() { return 7; }
template <class T> struct X {
  int v;
  X() : v(1) {}
  X(const X &o) : v(o.v + 10) {}
  ~X() { printf("~X\n"); }
  int m() const { return v + 100; }
  operator long() const { return 5; }
  static void *operator new(unsigned long n) { printf("new\n"); return malloc(n); }
  static void operator delete(void *p) { printf("del\n"); free(p); }
  static int sv;
  X operator+(const X &) const { return X(); }
};
template <class T> int X<T>::sv = 40;
template <class T> int vt = 50;
struct P { int a; };
inline P mk() { return P{3}; }
template <class T> int g() {
  X<int> x;
  X<int> y = x;
  X<int> *p = new X<int>;
  delete p;
  long l = x;
  int r = f2<int>() + h() + x.m() + y.v + (int)l + X<int>::sv + vt<short> + mk().a;
  r += (x + y).v;
  return r;
}
int main() { printf("%d\n", g<char>()); }
