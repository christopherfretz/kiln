export module imgcs;
export namespace cs {
template <class T> struct X { static constexpr int v = 1; };
template <class T> struct X<T *> {
  static constexpr int v = 2;
  T *p;
  constexpr X &self() { return *this; }
  constexpr int get() const { return p ? *p : -1; }
};
template <class T> struct X<const T> { static constexpr int v = 3; };
template <> struct X<int> {
  static constexpr int v = 4;
  int f() const { return 40; }
};
template <class T, int N> struct A { static constexpr int v = N; };
template <int N> struct A<char, N> { static constexpr int v = N * 10; };
template <class T> struct A<T, 0> { static constexpr int v = -1; };
template <> struct A<long, 7> { static constexpr int v = 77; };
template <class... Ts> struct L { static constexpr int v = sizeof...(Ts); };
template <class T, class... Ts> struct L<int, T, Ts...> { static constexpr int v = 100 + sizeof...(Ts); };
template <> struct L<> { static constexpr int v = -5; };
template <class T> struct B;
template <class T> struct B<T &> { using type = T; };
template <> struct B<void>;
template <class T> struct F { static constexpr int v = 0; };
template <class T> struct F<T *>;
template <class T> struct F<T *> { static constexpr int v = 8; };
constexpr int used = X<char *>::v + X<const char>::v;
constexpr int g() { return X<short *>{}.get() + X<int>::v; }
template <class T> struct E { };
template <class T> struct E<T *> { static_assert(sizeof(T) > 1, "E<T*> needs a wide T"); };
}
namespace cs {
template <class T> struct X<T &> { static constexpr int v = 5; };
template <> struct X<char> { static constexpr int v = 6; };
}
template <class T> struct cs::A<T, 1> { static constexpr int v = 11; };
template <> struct cs::A<int, 2> { static constexpr int v = 22; };
