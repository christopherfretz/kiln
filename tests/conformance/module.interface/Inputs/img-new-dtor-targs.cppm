export module imgndt;
export namespace ndt {
using size_t = decltype(sizeof 0);
struct tag {};
}
export inline void *operator new(ndt::size_t, ndt::tag, void *p) noexcept {
  return p;
}
export namespace ndt {
template <class T> T &&dv() noexcept;
template <bool B> struct bool_constant {
  static constexpr bool value = B;
};
template <class T, class... A>
struct nothrow_new
    : bool_constant<noexcept(::new (tag{}, dv<void *>()) T(dv<A>()...))> {};
template <class T>
struct nothrow_dtor : bool_constant<noexcept(dv<T &>().~T())> {};
template <class T> void destroy(T *p) { p->~T(); }
template <class T, int N> constexpr int size() { return int(sizeof(T)) * N; }
inline int sizes() { return size<int, 2>() + size<char, 3>(); }
template <class T> T *make(T v) { return new T(v); }
inline int *boxed(int v) { return new int{v}; }
} // namespace ndt
