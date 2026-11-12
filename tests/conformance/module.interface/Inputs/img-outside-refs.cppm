export module imgout;
export namespace ou {
template <class T> struct counted {
  constexpr T &get() noexcept(noexcept(*cur)) { return *cur; }
  T *cur;
};
inline int made() {
  int x = 2;
  counted<int> c{&x};
  return c.get();
}
template <class D> struct base {
  constexpr int twice() const { return 2 * static_cast<const D *>(this)->one(); }
};
struct derived : base<derived> {
  constexpr int one() const { return 21; }
};
struct other : base<other> {
  constexpr int one() const { return 1; }
};
struct cmp {
  constexpr bool operator()(int a, int b) const { return a < b; }
};
struct fn {
  static constexpr auto go(int a, int b) noexcept(noexcept(cmp()(a, b)))
      -> decltype(cmp()(a, b)) {
    return cmp()(a, b);
  }
};
template <class T> concept addable = requires(T t) { t + 1; };
static_assert(addable<int>);
template <class T> constexpr auto inc(T t);
template <class T> constexpr int via(T t) { return int(inc(t)); }
template <class T> constexpr auto inc(T t) { return t + 1; }
template <class T> void need(T) requires addable<T> {}
} // namespace ou
