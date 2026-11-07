// [class.union]/2, [expr.const] (P3726R2, C++26): when an element of a
// union's active array member is constructed, the elements not yet
// constructed are inactive union elemental subobjects; they are not part
// of the value of a constant, so an inplace_vector-like class with a
// partly filled `union { T data[N]; }` is a valid constant, and copies of
// a trivially copyable union holding such an array (copy construction and
// assignment) keep the constructed elements.
//@ skip(clang): clang 22 has no P3074/P3726 (trivial unions, inactive union elemental subobjects)
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
} // namespace std

template <class T, int N> struct IV {
  union { T data[N]; };
  int n = 0;
  constexpr IV() {}
  constexpr IV(const IV &o) : n(o.n) {
    for (int i = 0; i < n; ++i)
      std::construct_at(&data[i], o.data[i]);
  }
  constexpr void push(T x) { std::construct_at(&data[n], x); ++n; }
};
constexpr IV<int, 4> mk() { IV<int, 4> v; v.push(1); v.push(2); return v; }
constexpr IV<int, 4> c = mk();
static_assert(c.n == 2 && c.data[0] == 1 && c.data[1] == 2);
constexpr IV<int, 4> c2 = c;
static_assert(c2.n == 2 && c2.data[1] == 2);

// Elements of class type, and an empty active array member.
struct P { int x; constexpr P(int v) : x(v) {} };
constexpr IV<P, 3> mkp() { IV<P, 3> v; v.push(P(5)); return v; }
constexpr IV<P, 3> cp = mkp();
static_assert(cp.data[0].x == 5);
constexpr IV<P, 3> ce{};
static_assert(ce.n == 0);

// Nested arrays: the rule applies to elements of elements.
union NU { int m[2][2]; char c; };
struct NH { NU u; };
constexpr NH mkn() { NH h; std::construct_at(&h.u.m[1][0], 8); return h; }
constexpr NH nh = mkn();
static_assert(nh.u.m[1][0] == 8);

// A trivially copyable union: copy construction and assignment.
union TU { int a[4]; char c; };
struct H { TU u; int n; };
constexpr H mk2() { H h; h.n = 0; std::construct_at(&h.u.a[1], 7); return h; }
constexpr H h2 = mk2();
static_assert(h2.u.a[1] == 7);
constexpr int copy() { H h = mk2(); H g = h; return g.u.a[1]; }
static_assert(copy() == 7);
constexpr int assign() { H h = mk2(); H g; g = h; return g.u.a[1]; }
static_assert(assign() == 7);
