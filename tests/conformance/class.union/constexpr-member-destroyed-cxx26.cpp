// [basic.life]/1, /7, [class.union.general]/2, [expr.const]/10.9 (C++26):
// destroying the active member of a union ends that member's lifetime and
// leaves it the union's last active member. Reading it is a read of an
// object outside its lifetime; reading another member names it as the
// active member (clang's wording); constructing a member anew makes that
// one active. A union whose member was never constructed has no active
// member. Siblings: a pseudo-destructor call on a scalar member, an array
// member destroyed element by element, a member of class type with a
// trivial destructor (spec/oven.md "C++26 union lifetime").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:67 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 65:15 note "in call to 'pseudo_destroyed()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:126 note "read of member 's' of union with active member 'c' is not allowed in a constant expression"
//@ diag: 67:15 note "in call to 'old_after_new()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:77 note "read of member 'c' of union with active member 's' is not allowed in a constant expression"
//@ diag: 69:15 note "in call to 'other_member()'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:134 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 71:15 note "in call to 'elements_destroyed()'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:105 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 73:15 note "in call to 'trivial_class()'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:54 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 74:72 note "in call to 'destroy_at<int>(&u.s)'"
//@ diag: 75:15 note "in call to 'destroyed_twice()'"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:44 note "read of member 'x' of union with no active member is not allowed in a constant expression"
//@ diag: 78:15 note "in call to 'never_active()'"
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std
using I = int;
struct Pair { int a; int b; };
union U {
  constexpr U() : c() {}
  constexpr ~U() {}
  long c;
  int s;
  int arr[2];
  Pair t;
};

// Accepted: a new member after the destruction, of the same kind or not.
constexpr int same_again() { U u; std::construct_at(&u.s, 1); std::destroy_at(&u.s); std::construct_at(&u.s, 5); return u.s; }
static_assert(same_again() == 5);
constexpr long other_after() { U u; std::construct_at(&u.s, 1); std::destroy_at(&u.s); std::construct_at(&u.c, 6L); return u.c; }
static_assert(other_after() == 6);
constexpr int assigned_after() { U u; u.s = 1; u.s.~I(); u.s = 3; return u.s; }
static_assert(assigned_after() == 3);
constexpr int live_element() { U u; u.arr[0] = 1; u.arr[1] = 2; std::destroy_at(&u.arr[0]); return u.arr[1]; }
static_assert(live_element() == 2);

// Rejected.
constexpr int pseudo_destroyed() { U u; u.s = 1; u.s.~I(); return u.s; }
static_assert(pseudo_destroyed() == 1);
constexpr long old_after_new() { U u; std::construct_at(&u.s, 1); std::destroy_at(&u.s); std::construct_at(&u.c, 6L); return u.s; }
static_assert(old_after_new() == 1);
constexpr long other_member() { U u; u.s = 1; std::destroy_at(&u.s); return u.c; }
static_assert(other_member() == 1);
constexpr int elements_destroyed() { U u; u.arr[0] = 1; u.arr[1] = 2; std::destroy_at(&u.arr[0]); std::destroy_at(&u.arr[1]); return u.arr[1]; }
static_assert(elements_destroyed() == 2);
constexpr int trivial_class() { U u; std::construct_at(&u.t, Pair{1, 2}); std::destroy_at(&u.t); return u.t.b; }
static_assert(trivial_class() == 2);
constexpr int destroyed_twice() { U u; u.s = 1; std::destroy_at(&u.s); std::destroy_at(&u.s); return 1; }
static_assert(destroyed_twice() == 1);
union W { constexpr W() {} constexpr ~W() {} int x; };
constexpr int never_active() { W w; return w.x; }
static_assert(never_active() == 0);
