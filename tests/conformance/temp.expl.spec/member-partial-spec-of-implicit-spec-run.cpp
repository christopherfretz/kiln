// [temp.expl.spec]/16, [temp.spec.partial.general]/7: a partial
// specialization declared for a member template of an implicitly
// instantiated class template specialization
// (`template<> template<char c> struct O<int, char>::A<1, c>`) has no
// enclosing template levels; its own parameters are its only arguments.
// One use picks the partial specialization and reads its parameter, the
// others pick the primary; compile-time and run-time values agree.
//@ expect: run
//@ std: c++20
//@ stdout: 195 195 | 2 0 | 390 2
#include <cstdio>
template <class... Ts> struct O {
  using First = int;
  template <Ts... a> struct A { static constexpr int v = 0; };
};
template <> template <char c> struct O<int, char>::A<1, c> {
  static constexpr int v = 2;
  static constexpr char w = c;
  First f = c + 1;
  constexpr int get() const { return f + w; }
};
template <class T> struct P {
  template <class U, int n> struct B { static constexpr int v = 0; };
};
template <> template <class U> struct P<long>::B<U*, 3> {
  static constexpr int v = sizeof(U);
  U u{};
};
template <class T> struct Q {
  template <class U> struct C { static constexpr int v = 0; };
};
template <class T> template <class U> struct Q<T>::C<U&> { static constexpr int v = sizeof(T) + sizeof(U); };
static_assert(O<int, char>::A<1, 'x'>::v == 2);
static_assert(O<int, char>::A<1, 'x'>::w == 'x');
static_assert(O<int, char>::A<2, 'x'>::v == 0);
static_assert(O<long, char>::A<1, 'x'>::v == 0);
static_assert(O<int, char>::A<1, 'a'>{}.get() == 'a' + 1 + 'a');
static_assert(P<long>::B<double*, 3>::v == sizeof(double));
static_assert(P<long>::B<double*, 4>::v == 0 && P<int>::B<double*, 3>::v == 0);
static_assert(Q<char>::C<int&>::v == 1 + sizeof(int) && Q<char>::C<int>::v == 0);
int main() {
  O<int, char>::A<1, 'a'> a;
  constexpr int ct = O<int, char>::A<1, 'a'>{}.get();
  std::printf("%d %d | %d %d | %d %d\n", ct, a.get(), O<int, char>::A<1, 'b'>::v, O<int, char>::A<3, 'b'>::v,
              a.get() * 2, (int)P<long>::B<short*, 3>{}.u + P<long>::B<short*, 3>::v);
}
