// [class.union.anon]/1: anonymous union and struct members are accessed as members of the enclosing object
//@ expect: compile-ok
struct V { union { int x; float y; }; int z; };
V v = {1, 2};
struct W { union { struct { int p, q; }; long r; }; };
static_assert(sizeof(W) == 8);
int k(W w) { return w.p + w.q + v.x + v.z; }
static union { int g; long h; };
namespace { union { int ns_member; }; }
int m() {
  union { int a; char b; };
  a = 1;
  static union { int u; };
  u = 2;
  return a + u + g + ns_member;
}
