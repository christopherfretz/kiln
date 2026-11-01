// [dcl.enum]/5: before the closing brace, the enumerators of an enumeration
// with a fixed underlying type have that underlying type, so `a | b` in a
// later initializer is the built-in `|` even when an `operator|` for the
// enumeration is declared (libc++ <charconv> `chars_format::general =
// fixed | scientific`). After the brace they have the enumeration's type.
// Verified against clang 19.
//@ expect: compile-ok
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
enum class E;
constexpr E operator|(E, E);
enum class E { a = 1, b = 2, c = a | b, check = same<decltype(a), int>::value };
constexpr E operator|(E x, E y) { return E(int(x) | int(y)); }
static_assert(int(E::c) == 3 && int(E::check) == 1);
static_assert(same<decltype(E::c), E>::value);
enum F : unsigned char { f0 = 1, f1 = f0 << 1, fcheck = same<decltype(f0), unsigned char>::value };
static_assert(f1 == 2 && fcheck == 1);
static_assert(same<decltype(f1), F>::value);
