// [basic.life]/7, [class.union.general]/2, [expr.const]/10.9: a glvalue
// or pointer formed from a union member designates that member, even
// when another member of the same type lies at the same address. While
// `a` is active, reading or assigning `b` through a reference or pointer
// bound to `u.b` is an access of an object outside its lifetime, and
// `std::construct_at(&u.b, ...)` makes `b` the active member. A read of
// the active member through a glvalue of a similar type stays accepted,
// the two addresses compare equal ([expr.eq]), and reading a common
// initial sequence through the inactive member is rejected in a constant
// expression ([class.mem.general]/26 permits it at run time only)
// (spec/oven.md "Union member designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:72 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'inactive_ref()'"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:73 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 64:15 note "in call to 'inactive_ptr()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:58 note "assignment to member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 66:15 note "in call to 'write_ref()'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:60 note "assignment to member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 68:15 note "in call to 'write_ptr()'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:85 note "read of member 'a' of union with active member 'b' is not allowed in a constant expression"
//@ diag: 70:15 note "in call to 'constructed_other()'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:60 note "read of member 's2' of union with active member 's1' is not allowed in a constant expression"
//@ diag: 72:15 note "in call to 'common_initial()'"
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
} // namespace std
union U { int a; int b; };
template <class T> constexpr bool is_null(const T *const &p) { return p == nullptr; }
union P { int *p; const int *q; };
struct S1 { int k; int x; };
struct S2 { int k; long y; };
union V { S1 s1; S2 s2; };

// Accepted.
constexpr int active_ref() { U u{.a = 1}; const int &r = u.a; return r; }
static_assert(active_ref() == 1);
constexpr bool same_address() { U u{.a = 1}; return &u.a == &u.b; }
static_assert(same_address());
constexpr bool same_address_void() { U u{.a = 1}; const void *p = &u.b; return p == &u.a; }
static_assert(same_address_void());
constexpr bool similar_type() { P u{.p = nullptr}; return is_null(u.p); }
static_assert(similar_type());
constexpr int constructed() { U u{.a = 1}; std::construct_at(&u.b, 7); return u.b; }
static_assert(constructed() == 7);
constexpr int assigned_member() { U u{.a = 1}; u.b = 5; const int &r = u.b; return r; }
static_assert(assigned_member() == 5);

// Rejected.
constexpr int inactive_ref() { U u{.a = 1}; const int &r = u.b; return r; }
static_assert(inactive_ref() == 1);
constexpr int inactive_ptr() { U u{.a = 1}; const int *p = &u.b; return *p; }
static_assert(inactive_ptr() == 1);
constexpr int write_ref() { U u{.a = 1}; int &r = u.b; r = 5; return u.a; }
static_assert(write_ref() == 5);
constexpr int write_ptr() { U u{.a = 1}; int *p = &u.b; *p = 5; return u.a; }
static_assert(write_ptr() == 5);
constexpr int constructed_other() { U u{.a = 1}; std::construct_at(&u.b, 7); return u.a; }
static_assert(constructed_other() == 7);
constexpr int common_initial() { V v{.s1 = {1, 2}}; return v.s2.k; }
static_assert(common_initial() == 1);
