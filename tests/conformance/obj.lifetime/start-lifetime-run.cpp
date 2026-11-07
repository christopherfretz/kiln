// [obj.lifetime] (P3726R2): at run time std::start_lifetime has no effect
// beyond evaluating its operand (GCC folds the builtin to `(void)arg`); in
// a constant it makes storage from std::allocator<T>::allocate usable as
// an implicit-lifetime aggregate. A constant whose union holds a partly
// constructed array is emitted with its unconstructed elements as zero
// bytes, and copies of it keep the constructed element.
//@ skip(clang): clang 22 has neither __builtin_start_lifetime nor P3074/P3726 (trivial unions)
//@ std: c++26
//@ expect: run
//@ strict-diags
//@ stdout: 7 2 5 1
#include <cstdio>
#include <memory>
namespace std {
template <class T> constexpr void start_lifetime(T &r) noexcept {
  __builtin_start_lifetime(__builtin_addressof(r));
}
} // namespace std

union TU { int a[4]; char c; };
struct H { TU u; int n; };
constexpr H mk() {
  H h;
  h.n = 2;
  std::construct_at(&h.u.a[1], 7);
  return h;
}
constexpr H h = mk();

struct S { int v; };
union U { S a[3]; char c; };
int calls;
U *get(U *p) { ++calls; return p; }

struct A { int x; int y; };
constexpr int alloc() {
  std::allocator<A> al;
  A *p = al.allocate(1);
  std::start_lifetime(*p);
  p->x = 3;
  p->y = 4;
  int r = p->x + p->y;
  al.deallocate(p, 1);
  return r;
}
static_assert(alloc() == 7);

int main() {
  U u;
  u.c = 1;
  std::start_lifetime(get(&u)->a);
  u.a[1].v = 5;
  H c = h;
  std::printf("%d %d %d %d\n", h.u.a[1], c.n, u.a[1].v, calls);
}
