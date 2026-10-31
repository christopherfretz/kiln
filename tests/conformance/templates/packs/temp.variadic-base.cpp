// [temp.variadic]/5: expansion in a base-specifier-list and mem-initializer-list.
template <class... Bs> struct D : Bs... {
  constexpr D() : Bs()... {}
};
struct X { int x = 1; };
struct Y { int y = 2; };
constexpr D<X, Y> d;
static_assert(d.x + d.y == 3);
static_assert(__is_base_of(X, D<X, Y>));
constexpr D<> e;
