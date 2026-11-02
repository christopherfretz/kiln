// [meta.reflection] P2996R13: std::meta::info is a scalar type whose
// values are produced by the compiler; == and != compare the designated
// entities, it is usable as an NTTP and as a constexpr variable.
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
using info = decltype(^^int);

static_assert(^^int == ^^int);
static_assert(^^int != ^^long);
static_assert(^^const int != ^^int);

using I = int;
static_assert(^^I == ^^int); // spike: alias sugar is dropped

constexpr info r = ^^double;
constexpr info r2 = r;
static_assert(r == r2 && r == ^^double);

constexpr info null{};
static_assert(null == info{} && null != r);

template <info R> struct Tag { static constexpr info value = R; };
static_assert(Tag<^^int>::value == ^^int);
static_assert(Tag<^^char>::value != Tag<^^int>::value);

namespace N { int v; void f(); }
struct S { int m; };
static_assert(^^N != ^^N::v);
static_assert(^^N::v == ^^N::v);
static_assert(^^N::f != ^^N::v);
static_assert(^^S::m != ^^S);

consteval info pick(bool b) { return b ? ^^int : ^^char; }
// `^^int && x` would reflect `int&&` (the type-id is greedy): parenthesize.
static_assert(pick(true) == (^^int) && pick(false) == ^^char);
static_assert(sizeof(info) == 8);
