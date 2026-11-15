// [basic.life]/7.1, [class.union.general]/2, [expr.const]/10.9: after
// std::destroy_at ends the lifetime of a union's active member, a read of
// it through a glvalue of a similar type (`const int *const &` bound to an
// `int *`) is rejected. Not strict: kiln's note says the union has no
// active member where clang says "read of object outside its lifetime"
// (spec/oven.md "Unions", destruction of a union member).
//@ expect: compile-fail
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std
template <class T> constexpr bool is_null(const T *const &p) { return p == nullptr; }

struct Row { long n; int *p; };
union U { constexpr U() : c() {} constexpr ~U() {} long c; Row r; };

constexpr bool destroyed() {
  U u;
  std::construct_at(&u.r, Row{1, nullptr});
  std::destroy_at(&u.r);
  return is_null(u.r.p);
}
// The read of `p` in is_null (line 17) is the fault; the call is in
// destroyed() above.
static_assert(destroyed());
