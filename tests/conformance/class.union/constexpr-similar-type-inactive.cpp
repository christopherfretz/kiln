// [class.union.general]/2, [basic.lval]/11, [expr.const]/10.9: reading
// through a glvalue of a similar type (`const int *const &` bound to an
// `int *`) is still a read of the member it designates, so a read of an
// inactive member is rejected and the note names both the member read
// and the active one. spec/oven.md "Unions".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:71 note "read of member 'q' of union with active member 'r' is not allowed in a constant expression"
//@ diag: 27:10 note "in call to 'is_null<int>(u.q[0])'"
//@ diag: 29:15 note "in call to 'inactive()'"
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
} // namespace std
template <class T> constexpr bool is_null(const T *const &p) { return p == nullptr; }

struct Row { long n; int *p; };
union U { constexpr U() : c() {} constexpr ~U() {} long c; Row r; int *q[2]; };

constexpr bool inactive() {
  U u;
  std::construct_at(&u.r, Row{1, nullptr});
  return is_null(u.q[0]);
}
static_assert(inactive());
