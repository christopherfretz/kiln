// [basic.life]/1.4, /9, [expr.const]/5.18, [class.union.general]/6: outside std::allocator storage,
// an explicit destructor call (or pseudo-destructor call) ends the lifetime of a local, parameter,
// array element or `new` object; destroying it again, reading, assigning or calling a member
// function of it is not constant, nor is its implicit destruction at scope exit or by `delete`
// unless a placement new (std::construct_at) re-created it. Assigning a union member after another
// member's destruction makes it active. spec/oven.md "Object lifetime". Pinned to C++23 (P3726R2
// changes union member destruction in C++26: basic.life/union-member-destroyed-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:53 note "destruction of object outside its lifetime is not allowed in a constant expression"
//@ diag: 57:15 note "in call to 'double_destroy()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:56 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 60:15 note "in call to 'read_after()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:54 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 63:15 note "in call to 'assign_after()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:56 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 66:15 note "in call to 'call_after()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:34 note "destroying object 't' whose lifetime has already ended"
//@ diag: 69:15 note "in call to 'scope_end()'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:43 note "destroying object 's' whose lifetime has already ended"
//@ diag: 72:15 note "in call to 'scope_end_nontrivial()'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:26 note "destroying object 'x' whose lifetime has already ended"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:30 note "destroying object 'a[1]' whose lifetime has already ended"
//@ diag: 78:15 note "in call to 'element()'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:50 note "destroying object '{*new S#0}' whose lifetime has already ended"
//@ diag: 81:15 note "in call to 'del()'"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 83:64 note "destroying object '{*new S[2]#0}[0]' whose lifetime has already ended"
//@ diag: 84:15 note "in call to 'del_array()'"
//@ diag: 100:15 error "static assertion expression is not an integral constant expression"
//@ diag: 99:55 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 100:15 note "in call to 'union_read()'"
namespace std {
using size_t = decltype(sizeof 0);
template <class T> constexpr void destroy_at(T* p) { p->~T(); }
}
constexpr void* operator new(std::size_t, void* p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T* construct_at(T* p, A&&... a) { return ::new ((void*)p) T(static_cast<A&&>(a)...); }
}

struct S { int v; constexpr S(int x) : v(x) {} constexpr ~S() {} constexpr int get() const { return v; } };
struct T { int a, b; };
using I = int;

constexpr bool double_destroy() { S s(1); s.~S(); s.~S(); return true; }
static_assert(double_destroy());

constexpr int read_after() { T t{1, 2}; t.~T(); return t.a; }
static_assert(read_after() == 1);

constexpr bool assign_after() { int x = 1; x.~I(); x = 2; return true; }
static_assert(assign_after());

constexpr int call_after() { S s(1); s.~S(); int r = s.get(); std::construct_at(&s, 2); return r; }
static_assert(call_after() == 1);

constexpr bool scope_end() { { T t{1, 2}; t.~T(); } return true; }
static_assert(scope_end());

constexpr bool scope_end_nontrivial() { S s(1); s.~S(); return true; }
static_assert(scope_end_nontrivial());

constexpr bool param(int x) { x.~I(); return true; }
static_assert(param(1));

constexpr bool element() { T a[2] = {}; a[1].~T(); return true; }
static_assert(element());

constexpr bool del() { S* p = new S(1); p->~S(); delete p; return true; }
static_assert(del());

constexpr bool del_array() { S* p = new S[2]{1, 2}; p[0].~S(); delete[] p; return true; }
static_assert(del_array());

// Recreating the object makes the implicit destruction valid.
constexpr int recreate() { S s(1); s.~S(); std::construct_at(&s, 5); return s.get(); }
static_assert(recreate() == 5);

constexpr int recreate_trivial() { T t{1, 2}; std::destroy_at(&t); std::construct_at(&t, 3, 4); return t.a + t.b; }
static_assert(recreate_trivial() == 7);

// A union member's destruction leaves no active member; assigning
// another member activates it ([class.union.general]/6).
union U { int a; float b; };
constexpr int union_switch() { U u{1}; u.a.~I(); u.b = 2.0f; u.a = 3; return u.a; }
static_assert(union_switch() == 3);

constexpr int union_read() { U u{1}; u.a.~I(); return u.a; }
static_assert(union_read() == 1);

// A class whose member's lifetime ended is itself alive.
struct P { int a, b; };
constexpr int partial() { P p{1, 2}; p.a.~I(); return p.b; }
static_assert(partial() == 2);

constexpr bool heap_ok() { S* p = new S(1); p->~S(); std::construct_at(p, 2); delete p; return true; }
static_assert(heap_ok());
