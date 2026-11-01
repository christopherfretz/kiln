// [dcl.spec.auto.general]/11-12, [temp.dep.expr]/3: inside a class
// template, a static member function with a deduced return type may be
// used in a later member declaration. The use is type-dependent; the
// return type is deduced when the specialization's member definition is
// instantiated (libc++ `move_iterator`:
// `using iterator_concept = decltype(__get_iter_concept());`).
struct A {};
struct B {};
template <class X, class Y> struct same { static constexpr bool value = false; };
template <class X> struct same<X, X> { static constexpr bool value = true; };

template <class T> struct mi {
  static constexpr auto get() {
    if constexpr (sizeof(T) > 1)
      return A{};
    else
      return B{};
  }
  using concept_t = decltype(get());
  static auto s() { return T{}; }
  using s_t = decltype(mi<T>::s());
  static_assert(sizeof(decltype(get())) == 1);
  static constexpr int n = sizeof(s());
};
static_assert(same<mi<int>::concept_t, A>::value);
static_assert(same<mi<char>::concept_t, B>::value);
static_assert(same<mi<long>::s_t, long>::value);
static_assert(mi<short>::n == sizeof(short));
