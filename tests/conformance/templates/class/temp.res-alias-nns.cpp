// [temp.inst]/2, [expr.prim.id.qual]/1: a class template specialization
// named through an alias as a nested-name-specifier is instantiated.
template <class U> struct Inner { static constexpr int v = sizeof(U) * 2; };
struct A { template <class U> struct In { static constexpr int v = sizeof(U) * 3; }; };
template <class T> struct B {
  using I = Inner<T>;
  using J = typename A::template In<T>;
};
using I2 = Inner<int>;
static_assert(I2::v == 8);
static_assert(B<char>::I::v == 2);
static_assert(B<short>::J::v == 6);
