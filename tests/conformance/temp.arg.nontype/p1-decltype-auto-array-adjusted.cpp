// [temp.param]/10, [temp.arg.nontype]/1: the type of a `decltype(auto)`
// non-type template parameter is deduced from the argument; an array or
// function type is then adjusted to a pointer, as a declared parameter of
// that type is (kiln rejected `D<arr>` as "not implicitly convertible to
// 'int[3]'", WP-1740). A parenthesized id-expression deduces a reference.
// An array argument for a non-pointer parameter has no conversion
// ([expr.const]/12).
//@ std: c++20
template <decltype(auto) V> struct D { static constexpr decltype(V) v = V; };
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
int g[3];
int fn(int) { return 0; }
static_assert(same<decltype(D<g>::v), int *const>);
static_assert(same<decltype(D<(g)>::v), int (&)[3]>);
static_assert(same<decltype(D<fn>::v), int (*const)(int)>);
static_assert(D<g>::v == g);
int main() {}
