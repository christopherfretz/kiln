// Per-specialization entries (spec/ast.md §18.9.4, WP-2221): a dependent
// type the enclosing template's instantiation rebuilds still dependent
// (a generic lambda's body: `typename U::type` with the lambda's own
// parameter, `typename Pair<T, U>::type` with both levels) is resolved by
// the instantiation of the call operator specialization
// ([expr.prim.lambda.closure]/3), which records the row; so is
// `U::template rebind<T>` ([temp.names]/3). A member template's signature
// and body are substituted from the pattern by the member template
// specialization (`Box<A>::conv<B>`).
//@ written-refs: inst-rebuilt.refs
struct A { using type = int; template <class V> using rebind = V; };
struct B { using type = long; template <class V> using rebind = V *; };
template <class X, class Y> struct Pair { using type = typename Y::type; };
template <class T> struct Box {
  template <class U> typename Pair<T, U>::type conv(const U &) const {
    typename U::template rebind<T> r{};
    return typename T::type(sizeof r);
  }
};
template <class T> int lam(const T &) {
  auto f = []<class U>(const U &) {
    typename U::type v = 0;
    typename Pair<T, U>::type w = 0;
    typename U::template rebind<T> r{};
    return (int)(v + w + sizeof r);
  };
  auto g = [](auto x) {
    typename decltype(x)::type y = 0;
    typename decltype(x)::template rebind<T> z{};
    return (int)(y + sizeof z);
  };
  return f(B{}) + g(A{}) + g(B{});
}
int run() { Box<A> b; return (int)b.conv(B{}) + lam(A{}); }
