// [expr.new]/24, [expr.const]/5.18 (C++26, P2747R2): the non-allocating
// placement new of <new> is evaluable in a constant expression when its
// pointer designates storage suitable for the new object: elements of
// std::allocator<T>::allocate storage (one at a time; a whole-array form
// into it is rejected, see constexpr-placement.cpp), a transient heap
// array's element, an inactive union member (which it makes active), and
// an object with a const member; std::construct_at and
// std::ranges::construct_at are unchanged. The same functions run at run
// time with the same results (stdout pinned from clang 22; no oracle
// comparison while the default oracle is clang 19, which lacks P2747).
//@ expect: run
//@ oracle: none
//@ args: -stdlib=libc++
//@ requires: clang>=22
//@ stdout: 12 5 2 3 9 1
#include <cstdio>
#include <memory>
#include <new>

struct P {
  int x, y;
  constexpr P(int a, int b) : x(a), y(b) {}
};

constexpr int allocator_storage() {
  std::allocator<P> al;
  P *p = al.allocate(3);
  for (int i = 0; i < 3; ++i) ::new (p + i) P(i, i * 10);
  int s = p[0].x + p[1].y + p[2].x;
  for (int i = 0; i < 3; ++i) p[i].~P();
  al.deallocate(p, 3);
  return s;
}
static_assert(allocator_storage() == 12);

constexpr int heap_element() {
  int *p = new int[4];
  ::new (p + 2) int(5);
  int r = p[2];
  delete[] p;
  return r;
}
static_assert(heap_element() == 5);

struct A { int a; };
struct B { int b; };
union U { A a; B b; };
constexpr int inactive_member() {
  U u{.a = {1}};
  ::new (&u.b) B{2};
  return u.b.b;
}
static_assert(inactive_member() == 2);

struct C { const int c; };
constexpr int const_member() {
  C c{1};
  C *q = ::new (&c) C{3};
  return q->c;
}
static_assert(const_member() == 3);

constexpr int construct_at() {
  std::allocator<P> al;
  P *p = al.allocate(1);
  std::construct_at(p, 3, 4);
  int r = p->y;
  std::destroy_at(p);
  std::ranges::construct_at(p, 5, 6);
  r += p->x;
  std::ranges::destroy_at(p);
  al.deallocate(p, 1);
  return r;
}
static_assert(construct_at() == 9);

// Reused storage: 1000 objects in one slot.
constexpr int reuse() {
  std::allocator<P> al;
  P *p = al.allocate(1);
  int s = 0;
  for (int i = 0; i < 1000; ++i) {
    ::new (p) P(i, 1);
    s += p->y;
    p->~P();
  }
  al.deallocate(p, 1);
  return s / 1000;
}
static_assert(reuse() == 1);

int main() {
  std::printf("%d %d %d %d %d %d\n", allocator_storage(), heap_element(), inactive_member(),
              const_member(), construct_at(), reuse());
}
