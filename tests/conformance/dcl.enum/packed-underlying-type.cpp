// GNU extension: `__attribute__((packed))` on an enumeration without a
// fixed underlying type, written after `enum` or after the `}` (clang
// `ParseEnumBody` applies the trailing GNU attributes to the enumeration
// ahead of `ActOnEnumBody`), narrows its underlying type to the smallest
// of `unsigned char` / `unsigned short` (`signed char` / `short` with a
// negative value) that fits, else as without it; the promotion type stays
// `int` (clang `computeBestEnumTypes`). A fixed underlying type is kept;
// a member enumeration of a class template is instantiated with its
// attributes. (WP-432)
//@ expect: compile-ok
//@ strict-diags

enum E { A } __attribute__((packed));
static_assert(sizeof(E) == 1 && alignof(E) == 1);
static_assert(__is_same(__underlying_type(E), unsigned char));
enum F { B = 300 } __attribute__((packed));
static_assert(sizeof(F) == 2 && __is_same(__underlying_type(F), unsigned short));
enum __attribute__((packed)) G { C };
static_assert(sizeof(G) == 1);
enum N { M = -1 } __attribute__((packed));
static_assert(__is_same(__underlying_type(N), signed char));
enum N2 { M2 = -200, M3 = 100 } __attribute__((packed));
static_assert(__is_same(__underlying_type(N2), short));
enum U3 { U3a = 70000 } __attribute__((packed));
static_assert(sizeof(U3) == 4 && __is_same(__underlying_type(U3), unsigned int));
enum U4 { U4a = 255 } __attribute__((packed));
static_assert(sizeof(U4) == 1);
enum U5 { U5a = 65536 } __attribute__((packed));
static_assert(sizeof(U5) == 4);
enum S8 { S8a = -128, S8b = 127 } __attribute__((packed));
static_assert(__is_same(__underlying_type(S8), signed char));
enum S9 { S9a = -1, S9b = 128 } __attribute__((packed));
static_assert(__is_same(__underlying_type(S9), short));
enum class H { D } __attribute__((packed));
static_assert(sizeof(H) == 4);
enum __attribute__((packed)) W : int { Wa };
static_assert(sizeof(W) == 4);
template <class T> struct TT { enum In { X = sizeof(T) } __attribute__((packed)); };
static_assert(sizeof(TT<int>::In) == 1);
static_assert(sizeof(TT<char[300]>::In) == 2);
auto pr = +A;
static_assert(__is_same(decltype(pr), int));
struct WithE { char c; E e; F f; };
static_assert(sizeof(WithE) == 4);
enum V { V0 } __attribute__((packed)) vv, *vp;
static_assert(sizeof(vv) == 1);
