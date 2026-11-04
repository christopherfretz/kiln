// [over.match.funcs]/4-5, [class.mfct.non.static]/2, [dcl.spec.auto.general]:
// a recursive variadic tuple whose `get<I>()` and `get<I>() const` (with
// deduced `auto &` / `const auto &` return types) recurse through
// `Base::template get<I - 1>()`: the qualified template-id's implicit
// object argument is `*this`, so each level picks the overload of the
// same constness instead of finding the pair ambiguous (which, inside
// the return-type deduction of the enclosing `get`, would surface as "no
// matching member function"). WP-543.
//@ expect: run
//@ std: c++20
//@ exit: 9
template <class... Ts> struct tuple;
template <> struct tuple<> {};
template <class T, class... Ts> struct tuple<T, Ts...> : tuple<Ts...> {
  using Base = tuple<Ts...>;
  template <int I> auto &get() {
    if constexpr (I == 0)
      return value;
    else
      return Base::template get<I - 1>();
  }
  template <int I> const auto &get() const {
    if constexpr (I == 0)
      return value;
    else
      return Base::template get<I - 1>();
  }
  T value{};
};
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
int main() {
  tuple<int, long, short> t;
  t.get<0>() = 2;
  t.get<1>() = 3;
  t.get<2>() = 4;
  const auto &c = t;
  static_assert(same<decltype(t.get<2>()), short &>::value);
  static_assert(same<decltype(c.get<2>()), const short &>::value);
  return c.get<0>() + int(c.get<1>()) + c.get<2>();
}
