// [class.union.general], [basic.life] (P3074R7, P3726R2, C++26; WP-760):
// a construction (std::construct_at, placement new) of an element of the
// active array member of a union changes nothing, and the Oven remembers
// that for every element of the array (spec/oven.md "Union-path memo");
// once another member is active, constructing an element must activate
// the array again rather than be answered from that memo (GCC 17; see
// constexpr-trivial-union-lifetime-cxx26.cpp).
//@ skip(clang): clang 22 does not implement P3074/P3726 (trivial unions)
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  constexpr ~S() { delete p; }
};
union U { S arr[4]; int i; };
struct V {
  int n = 0;
  U u;
};

// Constructions of every element of the active array member are
// remembered as doing nothing; after `i` becomes active, constructing an
// element activates the array again.
constexpr int reactivate() {
  V v;
  for (int k = 0; k < 4; ++k)
    std::construct_at(&v.u.arr[k], k + 1);
  int r = 0;
  for (int round = 0; round < 3; ++round)
    for (int k = 0; k < 4; ++k) {
      r += *v.u.arr[k].p;
      std::destroy_at(&v.u.arr[k]);
      std::construct_at(&v.u.arr[k], k + 1);
    }
  for (int k = 0; k < 4; ++k)
    std::destroy_at(&v.u.arr[k]);
  v.u.i = 10;
  r += v.u.i;
  std::construct_at(&v.u.arr[2], 5);
  r += *v.u.arr[2].p;
  std::destroy_at(&v.u.arr[2]);
  return r;
}
static_assert(reactivate() == 45);
