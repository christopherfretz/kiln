// [expr.const]/5.18 before C++26: a placement new is evaluable only within
// namespace std (std::construct_at); elsewhere clang 22 names the language
// mode it needs (P2747R2 is a C++26 feature).
//@ expect: compile-fail
//@ std: c++23
//@ requires: clang>=22
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:38 note "this placement new expression is not supported in constant expressions before C++2c"
//@ diag: 20:15 note "in call to 'outside()'"
namespace std {
using size_t = decltype(sizeof 0);
template <class T> constexpr T *construct_at(T *p, int v) { return ::new ((void *)p) T(v); }
}
void *operator new(std::size_t, void *p) noexcept;

constexpr int in_std() { int x = 0; std::construct_at(&x, 3); return x; }
static_assert(in_std() == 3);
constexpr int outside() { int x = 0; new (&x) int(3); return x; }
static_assert(outside() == 3);
