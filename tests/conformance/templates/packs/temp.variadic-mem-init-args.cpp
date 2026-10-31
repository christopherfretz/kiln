// [temp.variadic]/5: a mem-initializer pack expansion with arguments.
template <class... Bs> struct D : Bs... {
  constexpr D(int v) : Bs(v)... {}
};
struct X { int x; constexpr X(int v) : x(v) {} };
struct Y { int y; constexpr Y(int v) : y(v * 2) {} };
constexpr D<X, Y> d(5);
static_assert(d.x == 5 && d.y == 10);
