// [temp.dep.type]/6, [temp.res.general]: a builtin template-id with a
// dependent argument, `__make_integer_seq<S, T, E>`, is a dependent type
// and so a dependent nested-name-specifier; a member named through it,
// `::n` or `::template to<S>`, is looked up at instantiation. libc++'s
// `__make_indices_imp` (`__utility/integer_sequence.h`). WP-121.
//@ expect: compile-ok

template <unsigned long...> struct TI {};
template <class I, I... V> struct IS {
  static constexpr int n = sizeof...(V);
  template <unsigned long S> using to = TI<(V + S)...>;
};

template <unsigned long E> constexpr int f() { return __make_integer_seq<IS, unsigned long, E>::n; }
static_assert(f<3>() == 3);
static_assert(f<0>() == 0);

template <unsigned long E, unsigned long S>
using indices = typename __make_integer_seq<IS, unsigned long, E - S>::template to<S>;
static_assert(__is_same(indices<2, 0>, TI<0, 1>));
static_assert(__is_same(indices<5, 2>, TI<2, 3, 4>));

template <class T, T N> struct W {
  using type = typename __make_integer_seq<IS, T, N>::template to<1>;
};
static_assert(__is_same(W<unsigned long, 2>::type, TI<1, 2>));

int main() {}
