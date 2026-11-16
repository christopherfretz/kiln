// [class.union.general]/6, [intro.object]/2, [expr.const] (P3726R2,
// C++26): a single-object placement new into an element of an inactive
// union array member (`new (&u.arr[0]) int(7)` while `x` is active)
// activates the array, as the array form does, and `x` is no longer
// active: reading it is rejected (found running GCC's C++ testsuite; GCC
// 16.1 agrees). `&u.x` designates `x` itself (spec/oven.md "Union member
// designators"), so constructing through it keeps or makes `x` active.
//@ skip(clang): clang 22 rejects constructing a subobject of an inactive union member
//@ oracle: gcc
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:69 note "read of member 'x' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'stale()'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:76 note "read of member 'x' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 47:15 note "in call to 'stale_second()'"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:81 note "read of member 'x' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 49:15 note "in call to 'stale_other_type()'"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:84 note "read of member 'arr' of union with active member 'x' is not allowed in a constant expression"
//@ diag: 51:15 note "in call to 'array_after_member()'"
using size_t = decltype(sizeof(0));
constexpr void *operator new(size_t, void *p) noexcept { return p; }

union U { int x; int arr[2]; };
union L { long x; int arr[2]; };

// Accepted.
constexpr int element() { U u{.x = 1}; new (&u.arr[0]) int(7); return u.arr[0]; }
static_assert(element() == 7);
constexpr int second_element() { U u{.x = 1}; new (&u.arr[1]) int(7); return u.arr[1]; }
static_assert(second_element() == 7);
constexpr int element_other_type() { L u{.x = 1}; new (&u.arr[0]) int(7); return u.arr[0]; }
static_assert(element_other_type() == 7);
constexpr int member_again() { U u{.x = 1}; new (&u.x) int(5); return u.x; }
static_assert(member_again() == 5);
constexpr int member_after_array() { U u{.arr = {1, 2}}; new (&u.x) int(5); return u.x; }
static_assert(member_after_array() == 5);

// Rejected.
constexpr int stale() { U u{.x = 1}; new (&u.arr[0]) int(7); return u.x; }
static_assert(stale() == 7);
constexpr int stale_second() { U u{.x = 1}; new (&u.arr[1]) int(7); return u.x; }
static_assert(stale_second() == 1);
constexpr long stale_other_type() { L u{.x = 1}; new (&u.arr[0]) int(7); return u.x; }
static_assert(stale_other_type() == 1);
constexpr int array_after_member() { U u{.arr = {1, 2}}; new (&u.x) int(5); return u.arr[1]; }
static_assert(array_after_member() == 2);
