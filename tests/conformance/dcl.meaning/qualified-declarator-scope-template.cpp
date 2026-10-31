// [basic.scope.class]/1, [temp.local]/7, [dcl.meaning]: after the
// declarator-id `C<X>::name` of an out-of-line member of a class
// template, the parameters, trailing return type, trailing
// requires-clause and initializer are looked up in the scope of `C`. A
// trailing requires-clause of a member names the class's members, and is
// checked in the specialization. `typename C<X>::T` before the
// declarator-id names the member of the current instantiation
// ([temp.dep.type]/6), so the redeclaration matches. WP-121.
template <class X> struct C {
  using T = X;
  X v;
  constexpr auto f() const -> decltype(v);
  constexpr T g(T) const;
  constexpr int p(T) const requires(sizeof(T) > 1);
  constexpr int p(T) const requires(sizeof(T) == 1);
  static const T z;
  static const T y;
  T h(T) const;
};
template <class X> constexpr auto C<X>::f() const -> decltype(v) { return v; }
template <class X> constexpr auto C<X>::g(T x) const -> T { return x + v; }
template <class X> constexpr int C<X>::p(T) const requires(sizeof(T) > 1) { return 2; }
template <class X> constexpr int C<X>::p(T) const requires(sizeof(T) == 1) { return 1; }
template <class X> const X C<X>::z = sizeof(T) * 2;
template <class X> const typename C<X>::T C<X>::y = z + 1;
template <class X> typename C<X>::T C<X>::h(typename C<X>::T t) const { return t; }
static_assert(C<int>{3}.f() == 3);
static_assert(C<int>::y == 9);
static_assert(__is_same(decltype(C<long>{}.h(0)), long));
static_assert(C<int>{3}.g(4) == 7);
static_assert(C<int>{}.p(0) == 2 && C<char>{}.p(0) == 1);
static_assert(C<int>::z == 8 && C<short>::z == 4);
