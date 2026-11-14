// Declaration forms a precompiled header's snapshot carries (WP-1353).
#ifndef DECL_FOLLOWUPS_H
#define DECL_FOLLOWUPS_H
namespace std { typedef decltype(sizeof 0) size_t; }
namespace f1 {
inline int *make() { return new int(7); }
}
namespace std { enum class align_val_t : size_t {}; }
namespace f1 {
inline std::size_t align_of(std::align_val_t a) { return static_cast<std::size_t>(a); }
}
namespace f2 {
struct no { static constexpr bool value = false; };
template <bool> struct pick;
template <> struct pick<true> {
  template <class R, class F, class... Rest>
  using result = typename pick<!bool(F::value) && sizeof...(Rest) != 0>::template result<F, Rest...>;
};
template <> struct pick<false> {
  template <class R, class...> using result = R;
};
template <class... A> using any = typename pick<sizeof...(A) != 0>::template result<no, A...>;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T *> { static constexpr bool value = true; };
template <class T> struct is_int { static constexpr bool value = false; };
template <> struct is_int<int> { static constexpr bool value = true; };
template <class T> struct ptr_or_int : any<is_ptr<T>, is_int<T>> {};
template <class T> concept pi = any<is_ptr<T>, is_int<T>>::value;
struct holder { template <class T> struct box { T v; }; };
template <class T> struct wrap { typename holder::template box<T> b; };
}
namespace f3 {
template <class T> struct old { T v; int get() const { return int(v); } };
extern template struct __attribute__((__deprecated__)) old<char>;
}
namespace f4 {
template <class D> struct adaptor {
  template <class X> constexpr int operator()(X x) const { return static_cast<const D &>(*this)(x, 1); }
};
struct drop;
struct drop : adaptor<drop> {
  constexpr int operator()(int a, int b) const { return a + b; }
  using adaptor<drop>::operator();
};
inline constexpr drop dr;
struct outer {
  struct base { virtual int k() const { return 0; } };
  template <class R> struct res : base { int k() const override { return 1; } };
};
template <> struct outer::res<void> : outer::base { int k() const override { return 3; } };
struct M { int x = 2; M(); M(const M &); ~M(); };
inline M::M() = default;
inline M::M(const M &) = default;
inline M::~M() = default;
}
#endif
