// [expr.prim.req.general]/4, [temp.variadic]/5: a requires-expression
// whose parameter list holds parameter packs: each pack becomes one
// parameter per element, mixed with ordinary parameters; a pack whose
// length differs from another in one expansion makes the requirement
// unsatisfied. Instantiating an enclosing template while the lambda's
// own level stays (`auto u`) keeps the requires-expression dependent
// until the call supplies it.
template <class... T> struct R {
  template <class... U> static constexpr bool c = requires(T... t, U... u) { (t + ... + 0); (u + ... + 0); };
  template <class... U> static constexpr bool d = requires(T... t, U... u) { ((t + u), ...); };
  template <class U> static constexpr bool g = requires(T... t, U u) { (t + ... + u); };
};
struct X {};
static_assert(R<int, int>::c<int, int>);
static_assert(R<int, int>::d<int, int>);
static_assert(!R<int, int>::d<int, X>);
static_assert(!R<int, int>::d<int>);
static_assert(R<int, int>::g<int>);
static_assert(!R<int, X>::g<int>);
template <class... T> constexpr bool a = requires(T... t) { sizeof...(t); };
static_assert(a<int, int> && a<>);
template <class... T> constexpr auto g() {
  return [](auto u) { return requires(T... t) { (t + ... + u); }; };
}
static_assert(g<int, int>()(1));
static_assert(!g<int, int>()(X{}));
static_assert(!g<X, int>()(1));
template <class... T> constexpr auto h() {
  return [](auto... u) { return requires(T... t, decltype(u)... v) { (t + ... + 0); requires sizeof...(v) == 2; }; };
}
struct Y { using type = int; };
static_assert(h<int, int>()(Y{}, Y{}));
static_assert(!h<int, int>()(Y{}));
static_assert(!h<X, int>()(Y{}, Y{}));
template <class A, class B> concept Same = __is_same(A, B);
template <class... T> constexpr auto k() {
  return [](auto u) { return requires(T... t) { { (t + ... + u) } -> Same<long>; }; };
}
static_assert(k<int, int>()(1L));
static_assert(!k<int, int>()(1));
