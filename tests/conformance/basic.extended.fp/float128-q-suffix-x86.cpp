//@ expect: compile-ok
//@ requires: target-x86_64
//@ std: c++20
//@ args: -std=gnu++20
// [lex.fcon] GNU extension: a `q`/`Q` floating-suffix types the literal
// `__float128` (clang `NumericLiteralParser::isFloat128`), distinct from
// `_Float128`. libstdc++'s `<numbers>` spells `__float128` constants this way
// whenever `__FLOAT128__` is predefined.
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

static_assert(same<decltype(1.5q), __float128>);
static_assert(same<decltype(2.718281828459045235360287471352662498Q), __float128>);
// (`_Float128` itself is not a C++ type name in clang 19/22, so the
// distinctness is checked against the standard types instead.)
static_assert(!same<decltype(1.5Q), long double>);
static_assert(!same<decltype(1.5Q), double>);
constexpr __float128 half = 0.5Q;
static_assert(half + half == 1.0Q);
