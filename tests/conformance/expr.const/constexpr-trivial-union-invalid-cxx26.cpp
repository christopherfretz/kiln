// [expr.const]/10, [class.union.general], [basic.life] (P3074R7, P3726R2,
// C++26): each invalid access to a trivial union in constant evaluation
// is diagnosed. Reading a member of a union with no active member (before
// any construction, or after the active member was destroyed); reading a
// member other than the active one; reading an array element that is a
// hole (its array is active, the element was never constructed or was
// destroyed); constructing an array element over the active member of
// another type without destroying it, which ends that member's lifetime
// without running its destructor, so its allocation leaks
// ([basic.life]/5). Expected output from the wording (spec/oven.md
// "Unions").
//@ skip(clang): clang 22 does not implement P3074/P3726 (trivial unions)
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:44 note "read of member 'i' of union with no active member is not allowed in a constant expression"
//@ diag: 51:15 note "in call to 'never_active()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:102 note "read of member 'i' of union with no active member is not allowed in a constant expression"
//@ diag: 54:15 note "in call to 'destroyed_then_read()'"
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:105 note "read of member 's' of union with no active member is not allowed in a constant expression"
//@ diag: 57:15 note "in call to 'destroyed_member_read()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:83 note "read of member 'i' of union with active member 's' is not allowed in a constant expression"
//@ diag: 60:15 note "in call to 'inactive_read()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:76 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 63:15 note "in call to 'hole_read()'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:11 note "read of member 'arr' of union with no active member is not allowed in a constant expression"
//@ diag: 71:15 note "in call to 'destroyed_element_read()'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:26 note "allocation performed here was not deallocated"
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

constexpr int never_active() { U u; return u.i; }
static_assert(never_active() == 0);

constexpr int destroyed_then_read() { U u; std::construct_at(&u.s, 7); std::destroy_at(&u.s); return u.i; }
static_assert(destroyed_then_read() == 0);

constexpr int destroyed_member_read() { U u; std::construct_at(&u.s, 7); std::destroy_at(&u.s); return *u.s.p; }
static_assert(destroyed_member_read() == 0);

constexpr int inactive_read() { U u; u.i = 1; std::construct_at(&u.s, 7); int r = u.i; std::destroy_at(&u.s); return r; }
static_assert(inactive_read() == 0);

constexpr int hole_read() { U u; std::construct_at(&u.arr[1], 4); int r = *u.arr[0].p; std::destroy_at(&u.arr[1]); return r; }
static_assert(hole_read() == 0);

constexpr int destroyed_element_read() {
  U u;
  std::construct_at(&u.arr[0], 4);
  std::destroy_at(&u.arr[0]);
  return *u.arr[0].p;
}
static_assert(destroyed_element_read() == 0);

constexpr int construct_over_active() { U u; std::construct_at(&u.s, 7); std::construct_at(&u.arr[0], 4); int r = *u.arr[0].p; std::destroy_at(&u.arr[0]); return r; }
static_assert(construct_over_active() == 4);
