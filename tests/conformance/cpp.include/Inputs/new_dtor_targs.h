// The shapes of <type_traits> `__is_nothrow_new_constructible_impl` and
// `__do_is_destructible_impl`: new-expressions, pseudo-destructor calls
// and calls with explicit template arguments in header bodies, some with
// implicitly declared allocation functions.
#ifndef NEW_DTOR_TARGS_H
#define NEW_DTOR_TARGS_H
namespace std {
using size_t = decltype(sizeof 0);
}
inline void *operator new(std::size_t, void *p) noexcept { return p; }
namespace ndt {
template <class T> T &&dv() noexcept;
template <bool B> struct bool_constant {
  static constexpr bool value = B;
};
template <class T, class... A>
struct nothrow_new
    : bool_constant<noexcept(::new (dv<void *>()) T(dv<A>()...))> {};
template <class T>
struct nothrow_dtor : bool_constant<noexcept(dv<T &>().~T())> {};
template <class T> void destroy(T *p) { p->~T(); }
template <class T, int N> constexpr int size() { return int(sizeof(T)) * N; }
inline int sizes() { return size<int, 2>() + size<char, 3>(); }
template <class T> T *make(T v) { return new T(v); }
template <class T> T *array(int n) { return new T[n]{}; }
inline int *boxed(int v) { return new int{v}; }
inline int *placed(void *p) { return ::new (p) int(4); }
struct counted {
  int a;
  counted() : a(5) {}
};
inline counted *fresh() { return new counted; }
} // namespace ndt
#endif
