// [over.over]/3, [temp.arg.explicit]/4: a template-id naming a member
// function template of a dependent class (`E<I>::template f<F>`), used
// without a call, names its one specialization once substituted, so it
// initializes a deduced return type or `auto` variable (libc++ variant's
// `return __dispatcher<_Is...>::template __dispatch<_Fp, _Vs...>;`). Called,
// the call still deduces the remaining arguments (a trailing pack) and sees
// every overload of the name; `&E<I>::template m<char>` is a pointer to
// member. WP-296.
//@ expect: compile-ok
template <int I> struct E {
  template <class F> static constexpr int f(F) { return I; }
  template <class F> static constexpr auto g(F) { return I + 10; }
  template <class F, class... R> static constexpr decltype(auto) h(F, R...) { return sizeof...(R); }
  template <class F> constexpr int m(F) const { return I + 20; }
  template <class F> static constexpr int k(F) { return 1; }
  template <class F> static constexpr int k(F, F) { return 2; }
};

template <class F, int I> constexpr auto f_of() { return E<I>::template f<F>; }
template <class F, int I> constexpr auto g_of() { return E<I>::template g<F>; }
template <class F, int... Is> constexpr auto h_of() {
  auto p = E<(Is + ...)>::template h<F>;
  return p;
}
template <int I> constexpr int call_h() { return E<I>::template h<int>(1, 2, 3); }
template <int I> constexpr int call_k() { return E<I>::template k<int>(1, 2) * 10 + E<I>::template k<int>(1); }
template <int I> constexpr auto member() { return &E<I>::template m<char>; }

static_assert(f_of<int, 1>()(0) == 1);
static_assert(g_of<long, 2>()(0) == 12);
static_assert(h_of<int, 1, 2>()(0) == 0);
static_assert(call_h<0>() == 2);
static_assert(call_k<0>() == 21);
static_assert((E<3>{}.*member<3>())('x') == 23);

int main() {}
