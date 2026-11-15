// [dcl.enum]/7: an enumeration without a fixed underlying type whose values
// no `int` holds (`-1` and `0x80000000`) gets an integral type that can
// represent them all: `long` on the LP64 targets (clang chooses the first
// of `int`, `long`, `long long` that fits). Before the closing brace each
// enumerator has the type of its initializer (`unsigned int` for
// `0x80000000`), after it the enumeration type ([dcl.enum]/5).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
enum C { c1 = -1, c2 = 0x80000000, c3 = sizeof(c2), c4 = __is_same(decltype(c2), unsigned) };
static_assert(sizeof(C) == 8);
static_assert(__is_same(__underlying_type(C), long));
static_assert(__is_same(decltype(c2), C));
static_assert(c3 == 4 && c4 == 1);
static_assert(c2 == 0x80000000L && c1 == -1);
static_assert(__is_same(decltype(+c1), long));
enum U { u1 = 0x80000000 };
static_assert(sizeof(U) == 4 && __is_same(__underlying_type(U), unsigned));
static_assert(__is_same(decltype(+u1), unsigned));
