// `-fshort-enums` (GCC and clang): every enumeration without a fixed
// underlying type gets the smallest integer type that holds its values,
// as `__attribute__((packed))` gives one ([dcl.enum]/7 leaves the choice
// to the implementation; clang `ActOnEnumBody`). A fixed underlying type
// is kept, and the promotion type is still `int` ([conv.prom]/3). kiln
// rejected the flag, so the run that needed it used 4-byte enumerations
// (found running GCC's C++ testsuite). WP-1461.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fshort-enums
enum A { a1 = 0x7fffffff };
enum B { b1 = 0x80000000 };
enum C { c1 = -1, c2 = 0x80000000 };
enum D { d1 = -128, d2 = 127 };
enum E { e1 = 0 };
enum F { f1 = 300 };
enum G { g1 = -200, g2 = 100 };
enum class H { h1 };
enum I : int { i1 };
static_assert(sizeof(A) == 4 && sizeof(B) == 4 && sizeof(C) == 8);
static_assert(sizeof(D) == 1 && sizeof(E) == 1 && sizeof(F) == 2 && sizeof(G) == 2);
static_assert(__is_same(__underlying_type(D), signed char));
static_assert(__is_same(__underlying_type(E), unsigned char));
static_assert(__is_same(__underlying_type(F), unsigned short));
static_assert(__is_same(__underlying_type(G), short));
static_assert(__is_same(__underlying_type(C), long));
static_assert(sizeof(H) == 4 && sizeof(I) == 4);
static_assert(__is_same(decltype(+e1), int) && __is_same(decltype(+f1), int));
template <class T>
struct S {
  enum M { m1 = sizeof(T) };
};
static_assert(sizeof(S<int>::M) == 1);
