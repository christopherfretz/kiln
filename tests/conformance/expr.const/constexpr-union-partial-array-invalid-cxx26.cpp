// [expr.const], [basic.indet] (P3726R2, C++26): an element of a union's
// array member that was never constructed is not part of the constant's
// value, so reading it (in the constant, or in a copy of it) is a read of
// an uninitialized object. An element that was constructed but left a
// member uninitialized is still an incomplete constant.
//@ skip(clang): clang 22 has no P3074/P3726 (trivial unions, inactive union elemental subobjects)
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:15 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:62 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 32:15 note "in call to 'copy_then_read()'"
//@ diag: 38:14 error "constexpr variable 'qh' must be initialized by a constant expression"
//@ diag: 37:68 note "subobject 'b' is not initialized"
//@ diag: 37:68 note "in call to 'QH(x)'"
//@ diag: 38:19 note "in call to 'mkq()'"
//@ diag: 34:23 note "subobject declared here"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
} // namespace std

union TU { int a[4]; char c; };
struct H { TU u; int n; };
constexpr H mk() { H h; h.n = 1; std::construct_at(&h.u.a[1], 7); return h; }
constexpr H h = mk();
static_assert(h.u.a[2] == 0);
constexpr int copy_then_read() { H x = mk(); H y = x; return y.u.a[0]; }
static_assert(copy_then_read() == 0);

struct Q { int a; int b; constexpr Q(int v) : a(v) {} };
union QU { Q q[2]; };
struct QH { QU u; };
constexpr QH mkq() { QH x; std::construct_at(&x.u.q[0], 1); return x; }
constexpr QH qh = mkq();
