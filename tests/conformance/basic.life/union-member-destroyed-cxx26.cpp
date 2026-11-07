// [basic.life]/1, [class.union.general] (P3726R2, C++26): destroying the
// active member of a union ends that member's lifetime but not the
// union's, which is left with no active member: it may be destroyed
// explicitly, or get a new member by construction or by assignment, and
// destroying the member again is destroying a member of a union with no
// active member. A member destroyed as an element of an active array
// ends only that element's lifetime (spec/oven.md "Unions").
//@ skip(clang): clang 22 does not implement P3726 (union lifetime)
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:58 note "destruction of member 's' of union with no active member is not allowed in a constant expression"
//@ diag: 57:89 note "in call to 'destroy_at<S>(&u.s)'"
//@ diag: 58:15 note "in call to 'destroy_twice()'"
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
union U { S s; int i; S arr[2]; };

constexpr int destroy_union_after() { U u; std::construct_at(&u.s, 1); std::destroy_at(&u.s); std::destroy_at(&u); return 1; }
static_assert(destroy_union_after() == 1);

constexpr int assign_after() { U u; std::construct_at(&u.s, 1); u.s.~S(); u.i = 4; return u.i; }
static_assert(assign_after() == 4);

constexpr int element_only() {
  U u;
  std::construct_at(&u.arr[0], 1);
  std::construct_at(&u.arr[1], 2);
  std::destroy_at(&u.arr[0]);
  int r = *u.arr[1].p;
  std::destroy_at(&u.arr[1]);
  return r;
}
static_assert(element_only() == 2);
constexpr int element_only_reversed() {
  U u;
  std::construct_at(&u.arr[1], 2);
  std::construct_at(&u.arr[0], 1);
  std::destroy_at(&u.arr[0]);
  int r = *u.arr[1].p;
  std::destroy_at(&u.arr[1]);
  return r;
}
static_assert(element_only_reversed() == 2);

constexpr int destroy_twice() { U u; std::construct_at(&u.s, 1); std::destroy_at(&u.s); std::destroy_at(&u.s); return 1; }
static_assert(destroy_twice() == 1);
