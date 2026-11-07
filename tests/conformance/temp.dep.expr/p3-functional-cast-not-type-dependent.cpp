//@ expect: run
//@ stdout: 7 7 8 1 2
// WP-782: [temp.dep.expr]/3: `simple-type-specifier ( expression-list )`
// is type-dependent only if its type is, even with a type-dependent
// operand. `RP d(u16(t.v))` therefore initializes `d` in the template
// definition by direct-initialization, choosing the explicit constructor
// (LLVM's CodeView `SimpleTypeSerializer`). `auto(t.v)` stays
// type-dependent until its placeholder is deduced.
#include <cstdio>

typedef unsigned short u16;
struct RP {
  explicit RP(u16 k) : k(k) {}
  u16 k;
};
struct A {
  int v;
};
struct B {
  long v;
};
int f(int) { return 1; }
int f(long) { return 2; }

template <class T> int direct(T t) {
  RP d(u16(t.v));
  return d.k;
}
template <class T> int braced(T t) {
  RP d{u16(t.v)};
  return d.k;
}
template <class T> int sized(T t) {
  RP d(u16(sizeof(t.v) * 2));
  return d.k;
}
template <class T> int call(T t) { return f(int(t.v)); }
template <class T> int placeholder(T t) { return f(auto(t.v)); }

int main() {
  A a{7};
  std::printf("%d %d %d %d %d\n", direct(a), braced(a), sized(a), call(a),
              placeholder(B{3}));
}
