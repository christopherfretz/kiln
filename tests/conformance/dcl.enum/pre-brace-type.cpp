// [dcl.enum]/5: before the closing brace of an enumeration without a fixed
// underlying type, an enumerator has the type of its initializing value (or
// of the previous enumerator, or `int` for the first one), so an operator
// on it inside the body is a built-in arithmetic operator even when
// operator functions are declared (the glibc `_ISwbit` pattern of
// <wctype.h> reached by <string_view>). After the brace it has the
// enumeration's type. Verified against clang 19.
//@ expect: compile-ok
struct S {};
S operator<<(S, int);
bool operator<(S, S);
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
enum {
  bit0 = 0,
  bit1 = 1,
  mask0 = ((bit0) < 8 ? (int)((1UL << (bit0)) << 24) : 0),
  mask1 = ((bit1) < 8 ? (int)((1UL << (bit1)) << 24) : 0),
  u = 3u,
  v,
  w = 'c',
  check_int = same<decltype(bit0), int>::value,
  check_uint = same<decltype(v), unsigned>::value,
  check_char = same<decltype(w), char>::value,
};
static_assert(check_int && check_uint && check_char);
static_assert(mask1 == (2 << 24));
enum E { e0, e1 = e0 + 1 };
static_assert(same<decltype(e1), E>::value);
static_assert(!same<decltype(bit0), int>::value);
