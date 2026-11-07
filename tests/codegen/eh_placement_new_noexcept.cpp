// [except.spec]/6, [expr.unary.noexcept]/3: a placement new-expression
// through the non-throwing `operator new(size_t, void*)` is
// potentially-throwing only through its initialization. A function
// whose noexcept-specifier asks `noexcept(::new (p) T(...))`, as the
// standard allocator's `construct` does, is therefore non-throwing for a
// T with a noexcept move constructor. It gets `nounwind`, and a caller
// with a live cleanup `call`s it instead of `invoke`ing it: no landing
// pad. It stays potentially-throwing for a T whose move may throw
// (WP-783).
namespace std { using size_t = decltype(sizeof 0); }
inline void *operator new(std::size_t, void *p) noexcept { return p; }
template <class T> T &&decl() noexcept;
struct M { M(M &&) noexcept; M(); ~M(); int x; };
struct Q { Q(Q &&); int y; };
template <class T>
void construct(T *p, T &&v) noexcept(noexcept(::new ((void *)0) T(decl<T>()))) {
  ::new ((void *)p) T(static_cast<T &&>(v));
}
struct Guard { ~Guard(); };
void take(void *);
void use_m(M *p, M &&v) {
  Guard g;
  construct(p, static_cast<M &&>(v));
  take(p);
}
void use_q(Q *p, Q &&v) {
  Guard g;
  construct(p, static_cast<Q &&>(v));
  take(p);
}
