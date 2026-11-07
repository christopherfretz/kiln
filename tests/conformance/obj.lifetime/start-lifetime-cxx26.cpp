// [obj.lifetime] (P3726R2, C++26): std::start_lifetime(r) begins the
// lifetime of the implicit-lifetime aggregate r without initializing it
// or any subobject; a union member becomes the active member. An object
// already within its lifetime is left as it is. kiln provides GCC's
// __builtin_start_lifetime (what libstdc++'s std::start_lifetime calls;
// libc++ has no std::start_lifetime yet, so the test declares it over the
// builtin). The array's elements stay outside their lifetime until
// constructed, and those not constructed are not part of the value of a
// constant ([expr.const], inactive union elemental subobjects).
//@ skip(clang): clang 22 has neither __builtin_start_lifetime nor P3074/P3726 (trivial unions)
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
template <class T> constexpr void start_lifetime(T &r) noexcept {
  __builtin_start_lifetime(__builtin_addressof(r));
}
} // namespace std

static_assert(__has_builtin(__builtin_start_lifetime));
static_assert(noexcept(__builtin_start_lifetime((int(*)[2])nullptr)));
static_assert(__is_same(decltype(__builtin_start_lifetime((int(*)[2])nullptr)), void));

struct S {
  int v;
  constexpr S(int x) : v(x) {}
};
union U { S a[3]; char c; };

// The array member becomes active with no element constructed.
constexpr int start_then_construct() {
  U u;
  std::start_lifetime(u.a);
  std::construct_at(&u.a[1], 4);
  return u.a[1].v;
}
static_assert(start_then_construct() == 4);

// Already the active member: no effect, constructed elements are kept.
constexpr int already_active() {
  U u;
  std::start_lifetime(u.a);
  std::construct_at(&u.a[0], 5);
  std::start_lifetime(u.a);
  return u.a[0].v;
}
static_assert(already_active() == 5);

// Another member was active: the array member replaces it.
constexpr int switch_member() {
  U u;
  u.c = 'x';
  std::start_lifetime(u.a);
  std::construct_at(&u.a[2], 9);
  return u.a[2].v;
}
static_assert(switch_member() == 9);

// Elements are constructed and destroyed one by one.
constexpr int per_element() {
  U u;
  std::start_lifetime(u.a);
  std::construct_at(&u.a[0], 1);
  std::construct_at(&u.a[1], 2);
  std::destroy_at(&u.a[0]);
  std::construct_at(&u.a[0], 3);
  return u.a[0].v * 10 + u.a[1].v;
}
static_assert(per_element() == 32);

// A constant holding the partly constructed array.
struct V { U u; int n; };
constexpr V make() {
  V v;
  v.n = 1;
  std::start_lifetime(v.u.a);
  std::construct_at(&v.u.a[0], 3);
  return v;
}
constexpr V cv = make();
static_assert(cv.u.a[0].v == 3 && cv.n == 1);

// A non-union aggregate already within its lifetime is left as it is.
struct P { int x, y; };
constexpr int plain() {
  P p{1, 2};
  std::start_lifetime(p);
  return p.x + p.y;
}
static_assert(plain() == 3);
