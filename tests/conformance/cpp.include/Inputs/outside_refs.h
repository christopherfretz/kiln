// Forms a precompiled header's image carries (WP-1360): bodies and types
// that name a declaration the walk records later.
#ifndef OUTSIDE_REFS_H
#define OUTSIDE_REFS_H
namespace o1 {
// A noexcept operand naming a member declared later in its class.
template <class T> struct counted {
  constexpr T &get() noexcept(noexcept(*cur)) { return *cur; }
  constexpr int left() const noexcept(noexcept(len + 1)) { return len; }
  T *cur;
  int len;
};
struct plain {
  int get() const noexcept(noexcept(v + 0)) { return v; }
  int v;
};
// A specialization the header instantiates before the operand is known.
inline int made() { counted<long> m{nullptr, 1}; return m.len; }
} // namespace o1
namespace o2 {
// A class naming itself in its own base clause.
template <class D> struct base {
  constexpr int twice() const { return 2 * static_cast<const D *>(this)->one(); }
};
struct derived : base<derived> {
  constexpr int one() const { return 21; }
};
template <class D> struct tag {};
struct both : base<both>, tag<both *> {
  constexpr int one() const { return 2; }
};
} // namespace o2
namespace o3 {
// An implicit special member that a later declaration's type names.
struct cmp {
  template <class T> constexpr bool operator()(T a, T b) const { return a < b; }
};
struct ord {
  constexpr ord(bool b) : v(b) {}
  bool v;
};
struct fn {
  template <class T>
  static constexpr auto go(T a, T b) noexcept(noexcept(ord(cmp()(a, b))))
      -> decltype(ord(cmp()(a, b))) {
    return ord(cmp()(a, b));
  }
};
} // namespace o3
namespace o4 {
// A requires-expression of a concept whose satisfaction was checked.
template <class A, class B> concept conv = requires(A a) { static_cast<B>(a); };
template <class T> concept testable = conv<T, bool> && requires(T &&t) {
  { !static_cast<T &&>(t) } -> conv<bool>;
};
static_assert(testable<int>);
// A data member named in an unevaluated operand of a member's type.
template <unsigned long N> struct raw { unsigned char d[N]; };
struct holder {
  void *p;
  raw<sizeof(p)> buf;
};
// A function template declared with a deduced return type, then defined.
template <class R, class V> constexpr auto conv_to(V v);
template <class V> constexpr int via(V v) { return conv_to<int>(v) + 1; }
template <class R, class V> constexpr auto conv_to(V v) { return R(v); }
} // namespace o4
namespace o5 {
// A member function template of a specialization named in a dependent
// call (the specialization's member record's pattern is a template).
template <bool> struct aux {
  template <class I> static constexpr int go(I f, I l) { return int(l - f); }
};
template <> struct aux<true> {
  template <class I> static constexpr int go(I, I) { return -1; }
};
template <class I> constexpr int span(I f, I l) {
  if (__builtin_is_constant_evaluated())
    return aux<false>::go(f, l);
  return aux<sizeof(I) == 1>::go(f, l);
}
} // namespace o5
#endif
