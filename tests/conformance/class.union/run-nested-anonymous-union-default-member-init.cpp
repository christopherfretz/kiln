// [class.union.general]/2, [class.default.ctor]/4: the implicit default
// constructor initializes the member of a nested anonymous union that has
// a default member initializer, at every level; a static object is
// constant-initialized with it ([basic.start.static]/2), and a
// mem-initializer for another variant member wins ([class.base.init]/9).
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ stdout: 5 7 1 2 5 7 1 2 9 5
extern "C" int printf(const char*, ...);
struct V { union { union { int x{5}; char y; }; long long z; }; };
struct W { union { union { union { int x{7}; char y; }; short s; }; long long z; }; };
struct A { union { struct { int a{1}; int b{2}; }; long long z; }; };
struct C {
  union { union { int x{5}; char y; }; long long z; };
  C() : z(9) {}
};
V gv;
W gw;
A ga;
int main() {
  V v;
  W w;
  A a;
  C c;
  // The bytes of the static object: `x` at offset 0.
  const unsigned char* p = reinterpret_cast<const unsigned char*>(&gv);
  int first = p[0] | p[1] << 8 | p[2] << 16 | p[3] << 24;
  int bytes = __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__ ? first : gv.x;
  printf("%d %d %d %d %d %d %d %d %lld %d\n", v.x, w.x, a.a, a.b, gv.x, gw.x, ga.a, ga.b, c.z, bytes);
}
