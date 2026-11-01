// [class.union.general]/6, [expr.new], [expr.const]/5.10 (clang 19
// findSubobject with AK_Construct): a placement new (std::construct_at)
// of an inactive union member makes it the active member; one of a
// subobject of an inactive member is not constant. kiln's pointers carry
// no designator, so members of one type at one offset may each be the
// constructed one (spec/oven.md "Unions", WP-233).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:86 note "read of member 'f' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 41:15 note "in call to 'previous_ended()'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:84 note "read of member 'd' of union with active member 'n' is not allowed in a constant expression"
//@ diag: 47:15 note "in call to 'class_member_then_other()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:103 note "read of member 'q' of union with active member 'p' is not allowed in a constant expression"
//@ diag: 67:15 note "in call to 'assigned_away()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:100 note "read of member 'c' of union with active member 'q' is not allowed in a constant expression"
//@ diag: 69:15 note "in call to 'assigned_same()'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:75 note "construction of subobject of member 'p' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 73:42 note "in call to 'construct_at<int, int>(&x.p.i, 9)'"
//@ diag: 74:15 note "in call to 'subobject()'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:75 note "construction of subobject of member 'p' of union with no active member is not allowed in a constant expression"
//@ diag: 76:44 note "in call to 'construct_at<int, int>(&y.p.i, 9)'"
//@ diag: 77:15 note "in call to 'subobject_no_active()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std

// The constructed member becomes the active one.
union U { int a; float f; };
constexpr int other_type() { U u{.f = 1.0f}; std::construct_at(&u.a, 5); return u.a; }
static_assert(other_type() == 5);
constexpr bool previous_ended() { U u{.f = 1.0f}; std::construct_at(&u.a, 5); return u.f == 1.0f; }
static_assert(previous_ended());
struct NT { int v; constexpr NT(int x) : v(x) {} constexpr ~NT() {} };
union Z { char d; NT n; constexpr Z() : d(0) {} constexpr ~Z() {} };
constexpr int class_member() { Z z; std::construct_at(&z.n, 11); int r = z.n.v; std::destroy_at(&z.n); return r; }
static_assert(class_member() == 11);
constexpr int class_member_then_other() { Z z; std::construct_at(&z.n, 11); return z.d; }
static_assert(class_member_then_other() == 0);
union E { int a; float f; constexpr E() {} };
constexpr int no_active() { E e; std::construct_at(&e.a, 6); return e.a; }
static_assert(no_active() == 6);
struct P { int j; int i; };
struct Q { int k; union { char c; P p; } u; };
constexpr int anonymous_in_class() { Q q{1, {.c = 'x'}}; std::construct_at(&q.u.p, P{3, 4}); return q.u.p.i + q.k; }
static_assert(anonymous_in_class() == 5);

// Members of one type at one offset: either may be the constructed one.
union S { int a; int b; };
constexpr int same_type() { S s{.a = 1}; std::construct_at(&s.b, 7); return s.b; }
static_assert(same_type() == 7);
constexpr int same_type_first() { S s{.b = 1}; std::construct_at(&s.a, 8); return s.a; }
static_assert(same_type_first() == 8);
union V { int x; float y; };
union W { V p; V q; char c; };
constexpr int same_union_type() { W w{.c = 1}; std::construct_at(&w.q, V{.x = 3}); return w.q.x; }
static_assert(same_union_type() == 3);
constexpr int assigned_away() { W w{.c = 1}; std::construct_at(&w.q, V{.x = 3}); w.p.y = 1.0f; return w.q.x; }
static_assert(assigned_away() == 3);
constexpr int assigned_same() { W w{.c = 1}; std::construct_at(&w.q, V{.x = 3}); w.q.x = 5; return w.c; }
static_assert(assigned_same() == 3);

// A subobject of an inactive member is not constructed.
union X { int a; P p; };
constexpr int subobject() { X x{.a = 1}; std::construct_at(&x.p.i, 9); return x.p.i; }
static_assert(subobject() == 9);
union Y { int a; P p; constexpr Y() {} };
constexpr int subobject_no_active() { Y y; std::construct_at(&y.p.i, 9); return y.p.i; }
static_assert(subobject_no_active() == 9);
