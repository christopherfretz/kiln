// [temp.param]/17, [temp.variadic]/5: a non-type template parameter whose
// type is a pack expansion over an enclosing template's type pack
// (`template<Ts... a>` in a member of `O<Ts...>`) becomes, once `Ts` is
// substituted, one non-type parameter per element: `O<int, char>::A`
// takes `<int, char>`, `O<>::A` takes none. [temp.param]/14: it is not
// itself a parameter pack, so parameters may follow it.
//@ expect: run
//@ std: c++20
template <class... Ts> struct O {
  template <Ts... a> struct A {
    static constexpr int n = sizeof...(a);
    static constexpr long sum() { return (0L + ... + long(a)); }
  };
  template <Ts... a, int n> struct B {
    static constexpr long v = (n + ... + long(a));
  };
  template <Ts... a> static constexpr long f() { return (0L + ... + long(a)); }
};
template <class... Ts> struct P {
  template <Ts... a> struct A {};
  template <class> struct Is { static constexpr int v = 0; };
  template <Ts... a> struct Is<A<a...>> { static constexpr int v = 1 + sizeof...(a); };
};
static_assert(O<int, char>::A<1, 'x'>::n == 2);
static_assert(O<int, char>::A<1, 'x'>::sum() == 1 + 'x');
static_assert(O<>::A<>::n == 0);
static_assert(O<int, int, int>::B<1, 2, 3, 10>::v == 16);
static_assert(O<>::B<5>::v == 5);
static_assert(O<int, char>::f<2, 'a'>() == 2 + 'a');
static_assert(O<>::f<>() == 0);
static_assert(P<int, char>::Is<P<int, char>::A<1, 'y'>>::v == 3);
static_assert(P<int, char>::Is<int>::v == 0);
int main() {
  O<short, long>::A<3, 4L> a;
  (void)a;
  return O<short, long>::A<3, 4L>::sum() == 7 && O<int>::f<9>() == 9 ? 0 : 1;
}
