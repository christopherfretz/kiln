// [expr.reflect], [basic.splice] (P2996): ^^T yields a std::meta::info and a
// splice of it names the same type.
//@ std: c++26
//@ xfail(clang): clang 19 does not implement P2996 reflection
using info = decltype(^^int);

constexpr info r = ^^int;
typename[:r:] x = 42;

static_assert(sizeof(x) == sizeof(int));
static_assert(^^int != ^^long);
