// [basic.life]/7.1, [class.union.general]/2, [expr.const]/10.9: after
// std::destroy_at ends the lifetime of a union's active member, a read of
// it through a glvalue of a similar type (`const int *const &` bound to an
// `int *`) is a read of an object outside its lifetime: the union keeps
// the ended member (spec/oven.md "C++26 union lifetime").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:71 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 29:10 note "in call to 'is_null<int>(u.r.p)'"
//@ diag: 32:15 note "in call to 'destroyed()'"
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
// The read of `p` in is_null (line 20) is the fault.
static_assert(destroyed());
