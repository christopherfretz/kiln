// [temp.mem]/1, [temp.var]: static data member templates of a class
// template, used from outside and from inside the class.
template <class T> struct A {
  template <class U> static constexpr int v = sizeof(T) + sizeof(U);
  template <int K> static constexpr T scaled = T(K) * 2;
  static constexpr int z = scaled<3>;
};
static_assert(A<int>::v<char> == 5);
static_assert(A<long>::v<long> == 16);
static_assert(A<int>::scaled<4> == 8);
static_assert(A<int>::z == 6);
