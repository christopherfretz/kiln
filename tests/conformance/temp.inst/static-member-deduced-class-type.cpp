// [temp.inst]/4, [dcl.type.class.deduct]/1: a static data member of a
// class template declared with a deduced class type (`arr const tbl =
// mk<F>();`) has its initializer instantiated with the class, as for
// `auto`, so its type is deduced before a member function uses it (clang
// instantiates the initializer of a variable with an undeduced type
// eagerly).
//@ verify
// expected-no-diagnostics
template <class T, unsigned N> struct arr {
  T d[N];
  constexpr unsigned size() const { return N; }
  constexpr const T& operator[](unsigned i) const { return d[i]; }
};
template <class T, class... U> arr(T, U...) -> arr<T, 1 + sizeof...(U)>;
template <class T> constexpr arr<T, 3> mk() { return {{T(1), T(2), T(3)}}; }

template <class F> struct P {
  static constexpr arr const tbl = mk<F>();
  static constexpr arr other{F(7), F(8)};
  static constexpr unsigned get() { return tbl.size() + other.size(); }
  static constexpr F at(unsigned i) { return tbl[i] + other[0]; }
};
static_assert(P<int>::get() == 5);
static_assert(P<long>::at(2) == 10);
