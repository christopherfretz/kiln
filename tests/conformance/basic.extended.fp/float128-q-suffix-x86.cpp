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
static_assert(!same<decltype(1.5Q), _Float128>);
constexpr __float128 half = 0.5Q;
static_assert(half + half == 1.0Q);
