export module imgdf;
export namespace df {
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
template <class T> struct ptr_or_ptr : any<is_ptr<T>, is_ptr<T *>> {};
template <class T> concept ptrish = any<is_ptr<T>>::value;
struct holder { template <class T> struct box { T v; }; };
template <class T> struct wrap { typename holder::template box<T> b; };
template <class T> struct old { T v; int get() const { return int(v); } };
extern template struct __attribute__((__deprecated__)) old<char>;
}
