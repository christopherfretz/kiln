// [temp.arg.explicit]/2, [temp.deduct.general]/2, [dcl.spec.auto.general]/3,
// [temp.inst]/3: a member function template of a class template whose
// return type is a placeholder constrained by the class's parameter
// (`template <int N> struct s { template <class T> of<N> auto get(); }`).
// Instantiating the class substitutes `N` into the type-constraint; the
// call `s<8>{}.get<double>()` then substitutes the explicit `T` into the
// return type. Before WP-1235 kiln kept the class's `N` in the member's
// return type (deduced from a dependent `return`), and the explicit
// argument substitution read it as the member's own parameter at the same
// depth: "not yet implemented: non-type template argument of this kind",
// and no viable `get`.
//@ std: c++20
//@ expect: run
//@ stdout: get 8 2
//@ stdout: pick 3
//@ stdout: two 5
//@ stdout: dep 8
extern "C" int printf(const char *, ...);

template <class T, auto V> concept of = sizeof(T) == V;
template <class T, auto V> concept rep_of = sizeof(T) >= V;

template <int N> struct s {
  template <class T> constexpr of<N> auto get() const { return T(2); }
  // The overload set of the reference shape: only the zero-argument
  // candidate is viable for `pick<T>()`.
  template <rep_of<N> T> constexpr of<N> auto pick() const { return T(3); }
  template <rep_of<N> T, class U> constexpr of<N> auto pick(U) const { return T(4); }
  template <class T, class U = int> constexpr of<N> auto two() const { return T(5); }
};

template <auto X> constexpr int via() { return int(s<X>{}.template get<double>()); }

int main() {
  constexpr auto a = s<8>{}.get<double>();
  printf("get %d %d\n", int(sizeof(a)), int(a));
  printf("pick %d\n", int(s<4>{}.pick<int>()));
  printf("two %d\n", int(s<4>{}.two<float>()));
  printf("dep %d\n", via<8>() * 4);
}
