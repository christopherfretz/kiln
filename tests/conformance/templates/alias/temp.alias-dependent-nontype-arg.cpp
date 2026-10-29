//@ expect: ast
//@ oracle: clang
// [temp.alias]/2, [temp.res.general]/6, [temp.arg.nontype]/2: an alias
// template named with a value-dependent non-type argument is substituted
// with the argument unconverted; it is converted and evaluated when the
// enclosing class template is instantiated.
template <bool B, class T = void> struct EnableIf { using type = T; };
template <bool B, class T = void> using EnableIfT = typename EnableIf<B, T>::type;
template <class P> struct ByType { EnableIfT<sizeof(P) == 1, int> x; };
template <int N> struct ByParam {
  EnableIfT<N == 1, long> a;
  EnableIfT<(N > 0), char> b;
};
static_assert(sizeof(ByType<char>) == sizeof(int));
static_assert(sizeof(ByParam<1>) == 2 * sizeof(long));
