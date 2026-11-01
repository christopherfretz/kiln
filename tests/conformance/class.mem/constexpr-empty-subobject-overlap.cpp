// [intro.object]/9, [class.mem.general]/23 ([[no_unique_address]]),
// [dcl.init.general]/6.2: an empty base or empty [[no_unique_address]]
// member may share its address with another subobject; value-initializing
// or copying it touches no byte of that subobject. libc++'s
// `__compressed_pair<pointer, default_delete<T>>` (std::unique_ptr)
// value-initializes its empty deleter base over the pointer.
//@ expect: compile-ok
struct E {};
struct B1 {
  int *v;
  constexpr B1(int *p) : v(p) {}
};
struct ZeroBase : B1, E {
  constexpr ZeroBase(int *p) : B1(p), E() {}
};
static_assert([] { int x = 5; ZeroBase r(&x); return *r.v; }() == 5);
struct CopyBase : B1, E {
  constexpr CopyBase(int *p, E e) : B1(p), E(e) {}
};
static_assert([] { int x = 6; CopyBase r(&x, E()); return *r.v; }() == 6);
struct PrivBase : private E {
  constexpr PrivBase() : E() {}
};
struct Pair : B1, PrivBase {
  constexpr Pair(int *p) : B1(p), PrivBase() {}
};
static_assert([] { int x = 7; Pair r(&x); return *static_cast<B1 &>(r).v; }() == 7);
struct Member {
  int *p;
  [[no_unique_address]] E e;
  constexpr Member(int *q) : p(q), e() {}
};
static_assert([] {
  int x = 8;
  Member m(&x);
  m.e = E();
  Member n = m;
  n.e = m.e;
  return *n.p;
}() == 8);
