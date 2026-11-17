// [basic.align]/1, [expr.alignof]/3: GNU `aligned` on an enumeration,
// before its name, after its closing brace, or on a typedef of it, raises
// the alignment `alignof` reports and the layout of a class holding it
// uses, never its size (clang 22 and GCC 16.1 agree in C++); a smaller
// value on a typedef lowers it (as for any typedef). An array of it whose
// element size is a multiple of the alignment is fine. The rejected side
// is over-aligned-enum-element-errors.cpp. Found running GCC's C++
// testsuite.
//@ std: c++26
//@ expect: compile-ok
enum __attribute__((aligned(8))) E : char { e };
typedef enum __attribute__((aligned(8))) F { f } AF;
enum G { g } __attribute__((aligned(16)));
enum H : short { h }; typedef enum H TH __attribute__((aligned(8)));
typedef enum H TL __attribute__((aligned(1)));
enum __attribute__((aligned(4))) Q : int { q };
static_assert(sizeof(E) == 1 && alignof(E) == 8);
static_assert(sizeof(AF) == 4 && alignof(AF) == 8 && alignof(F) == 8);
static_assert(sizeof(G) == 4 && alignof(G) == 16);
static_assert(sizeof(TH) == 2 && alignof(TH) == 8 && alignof(H) == 2);
static_assert(sizeof(TL) == 2 && alignof(TL) == 1);
struct S { char c; E x; char d; G y; };
static_assert(__builtin_offsetof(S, x) == 8 && __builtin_offsetof(S, y) == 16);
static_assert(sizeof(S) == 32 && alignof(S) == 16);
struct T { char c; TH x; };
static_assert(__builtin_offsetof(T, x) == 8 && sizeof(T) == 16);
Q qa[3];
TL tl[3];
static_assert(sizeof(qa) == 12 && alignof(decltype(qa)) == 4 && sizeof(tl) == 6);
constexpr E ce = e;
static_assert(alignof(decltype(ce)) == 8);
