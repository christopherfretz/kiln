// [temp.friend]/1, [temp.inst]/4, [class.mem.general]/7: a friend function
// template defined in a class template whose body names a specialization
// of that class template (`M<>{}`) instantiates the specialization while
// the body is parsed, before the body is attached; a later use of the
// friend from that specialization is still instantiated from the body (a
// units library's `pow<-1>(magnitude<>)` called from `operator/`).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags

template <int... Ms> struct M {
  friend consteval auto operator/(M, auto rhs) { return pow<-1>(rhs); }
  template <int Num> friend consteval auto pow(M) {
    if constexpr (Num == 0)
      return M<>{};
    else
      return M<(Ms * Num)...>{};
  }
};
static_assert(__is_same(decltype(M<>{} / M<>{}), M<>));
static_assert(__is_same(decltype(M<2>{} / M<3>{}), M<-3>));

template <class T> struct N {
  template <int K> friend consteval int size_plus(N) { return sizeof(N<int>) + K; }
  char c;
};
static_assert(size_plus<1>(N<int>{}) == 2);
static_assert(size_plus<2>(N<char>{}) == 3);

template <class T> constexpr bool divisible(T a, T b) { return sizeof(a / b) == 1; }
template <class T> concept C = divisible(T{}, T{});
static_assert(C<M<>>);
static_assert(C<M<4>>);
