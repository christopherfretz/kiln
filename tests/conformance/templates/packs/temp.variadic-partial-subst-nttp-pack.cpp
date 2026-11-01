// [temp.variadic]/5, [temp.alias]/2: a member alias template whose
// pattern names a non-type pack of the enclosing partial specialization
// (known) and a type pack of its own (still dependent):
// `All<O<T, I>...>` keeps `I` as a substituted pack until `T` is
// supplied (libc++ <variant>:1141, `__all_overloads`).
//@ expect: run
template <class T, unsigned long I> struct O {
  using type = T;
  static constexpr unsigned long idx = I;
};
template <class... B> struct All : B... {};
template <unsigned long... I> struct Seq {};
template <class S> struct Imp;
template <unsigned long... I> struct Imp<Seq<I...>> {
  template <class... T> using Apply = All<O<T, I>...>;
  template <class... T> static constexpr unsigned long sum = (0 + ... + (sizeof(T) * I));
};
using X = Imp<Seq<0, 1>>::Apply<int, char>;
static_assert(__is_same(X, All<O<int, 0>, O<char, 1>>));
static_assert(Imp<Seq<1, 2>>::sum<char, short> == 5);
static_assert(__is_same(Imp<Seq<>>::Apply<>, All<>));
int main() { return X().O<char, 1>::idx == 1 ? 0 : 1; }
