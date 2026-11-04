// [over.match.copy]/1.2, [over.match.conv]: the restriction of
// [class.conv.fct]/1 is to the class itself and its bases only; a class
// `E` unrelated to `D` (no constructor from `D`) is still initialized
// through `D`'s conversion function template `operator T()` with
// `T = E`, by copy-, direct- and direct-list-initialization (the last two
// through the CWG2327 candidates of clang's `ResolveConstructorOverload`)
// and as a by-value argument. Matches clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 42 42 42 42 9
extern "C" int printf(const char*, ...);
struct E {
  int v;
};
struct Base {};
struct D : Base {
  int w = 42;
  template <class T> operator T() const { return T{w}; }
};
struct DD : D {};
int take(E e) { return e.v; }
int main() {
  D d;
  E a = d;
  E b(d);
  E c{d};
  DD dd;
  dd.w = 9;
  E f = dd;  // through the base's conversion function template
  printf("%d %d %d %d %d\n", a.v, b.v, c.v, take(d), f.v);
  return 0;
}
