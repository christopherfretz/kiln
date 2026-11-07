// [class.union.general], [basic.life], [intro.object] (P3074R7, P3726R2,
// C++26): in constant evaluation a trivial union starts with no active
// member; std::construct_at (placement new) of a member activates it;
// destroying the active member (std::destroy_at, an explicit destructor
// call) leaves the union within its lifetime with no active member, so a
// member may be constructed or assigned anew; constructing an element of
// an inactive array member activates the array, whose other elements are
// holes with no lifetime (the inplace_vector pattern: per-element
// lifetime). Following GCC 17 rather than the DIS text, which needs
// std::start_lifetime for the array (spec/oven.md "Unions").
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
union U { S s; int i; S arr[3]; };

// Construct, destroy, then another member.
constexpr int cycle() {
  U u;
  std::construct_at(&u.s, 7);
  int r = *u.s.p;
  std::destroy_at(&u.s);
  u.i = 3;
  return r + u.i;
}
static_assert(cycle() == 10);

// An explicit destructor call ends the member; the union lives on.
constexpr int explicit_dtor() {
  U u;
  u.i = 1;
  std::construct_at(&u.s, 7);
  int r = *u.s.p;
  u.s.~S();
  std::construct_at(&u.s, 8);
  r += *u.s.p;
  u.s.~S();
  return r;
}
static_assert(explicit_dtor() == 15);

// An element of an inactive array member activates the array.
constexpr int array_element() {
  U u;
  std::construct_at(&u.arr[1], 4);
  int r = *u.arr[1].p;
  std::destroy_at(&u.arr[1]);
  std::construct_at(&u.arr[0], 5);
  r += *u.arr[0].p;
  std::destroy_at(&u.arr[0]);
  return r;
}
static_assert(array_element() == 9);

// The inplace_vector pattern, its storage in an anonymous union.
template <class T, int N> struct IV {
  union { T data[N]; };
  int n = 0;
  constexpr IV() {}
  constexpr ~IV() { for (int i = 0; i < n; ++i) std::destroy_at(&data[i]); }
  constexpr void push(int x) { std::construct_at(&data[n], x); ++n; }
  constexpr void pop() { --n; std::destroy_at(&data[n]); }
};
constexpr int vec() {
  IV<S, 4> v;
  v.push(1);
  v.push(2);
  v.pop();
  v.push(5);
  v.push(6);
  return *v.data[0].p + *v.data[1].p + *v.data[2].p + v.n;
}
static_assert(vec() == 15);

// Trivial elements, two-dimensional storage.
constexpr int grid() {
  union G { int g[2][3]; char c; } u;
  std::construct_at(&u.g[1][2], 6);
  std::construct_at(&u.g[0][0], 1);
  return u.g[1][2] + u.g[0][0];
}
static_assert(grid() == 7);

// A constexpr object whose union has no active member is a constant.
struct Opt { union { S s; }; bool has = false; constexpr Opt() {} };
constexpr Opt empty;
static_assert(!empty.has);
