//@ expect: run
// [temp.variadic]/7, [temp.inst]/3: a member template of a class
// template whose pattern expands an outer pack together with its own
// (`Conv<U, T>...`, T substituted, U not) keeps the expansion when the
// class is instantiated, expanded once U is known (clang's
// `SubstTemplateTypeParmPackType`; libc++ `tuple`'s converting
// constructors).
template <bool B> struct BC { static constexpr bool value = B; };
template <class T, class U> struct Conv : BC<__is_convertible(T, U)> {};
template <class... B> struct And : BC<(B::value && ...)> {};
template <bool B, class T = void> struct EI {};
template <class T> struct EI<true, T> { using type = T; };

template <class... T> struct Tup {
  template <class... U, typename EI<sizeof...(U) == sizeof...(T), int>::type = 0,
            typename EI<And<Conv<U, T>...>::value, int>::type = 0>
  static int make(U...) { return 1; }
  template <class... U> static constexpr bool all = And<Conv<U, T>...>::value;
  template <class... U> using First = typename EI<And<Conv<U*, T*>...>::value, char>::type;
  static int make(...) { return 0; }
};
struct A {};
static_assert(Tup<int, long>::all<char, short>);
static_assert(!Tup<int, A>::all<char, short>);
static_assert(sizeof(Tup<const int, const long>::First<int, long>) == 1);
int main() {
  if (Tup<int, long>::make('a', short(1)) != 1) return 1;
  if (Tup<int, A>::make(1, 2) != 0) return 2;
  if (Tup<int, long>::make(1) != 0) return 3;
  return 0;
}
