// [specialized.construct]/2, [expr.const]/5.18: std::construct_at(location,
// args...) constructs *location, so a member at offset 0 of its enclosing
// object (the first member of a struct, an optional's or variant's union
// storage) is the object created even when the library hands the placement
// new a plain `void *` (libc++'s `::new (std::__voidify(*location)) T(...)`).
// Header-free replica of libc++ 19's <__memory/construct_at.h>.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
namespace std {
template <class T> constexpr T *addressof(T &r) noexcept { return __builtin_addressof(r); }
template <class T> constexpr void *__voidify(T &from) {
  return const_cast<void *>(static_cast<const volatile void *>(std::addressof(from)));
}
template <class T, class... A>
constexpr T *construct_at(T *location, A &&...args) {
  return ::new (std::__voidify(*location)) T(static_cast<A &&>(args)...);
}
template <class T, class... A> constexpr T *__construct_at(T *location, A &&...args) {
  return std::construct_at(location, static_cast<A &&>(args)...);
}
} // namespace std

struct S { int a, b; };
constexpr int first_member() {
  S s{1, 2};
  std::construct_at(&s.a, 7);
  return s.a * 10 + s.b;
}
static_assert(first_member() == 72);

struct X { int a, b; constexpr X(int a, int b) : a(a), b(b) {} };
struct Wrapper { X x; };
struct Opt {
  union { char empty; Wrapper val; };
  bool engaged = false;
  constexpr Opt() : empty() {}
  constexpr void emplace(int a, int b) { std::__construct_at(&val, X(a, b)); engaged = true; }
};
struct Cache { Opt value; };
constexpr int optional_storage() {
  Cache c;
  c.value.emplace(3, 4);
  return c.value.val.x.a * 10 + c.value.val.x.b;
}
static_assert(optional_storage() == 34);

struct Var {
  union U { char none; X x; } u;
  int index = -1;
  constexpr Var() : u{} {}
};
constexpr int variant_storage() {
  Var v;
  std::construct_at(&v.u, Var::U{0});
  std::construct_at(&v.u.x, 5, 6);
  v.index = 1;
  return v.u.x.a + v.u.x.b;
}
static_assert(variant_storage() == 11);

int main() {}
