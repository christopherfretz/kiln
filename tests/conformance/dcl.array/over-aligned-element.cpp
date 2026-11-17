// [dcl.array]/1, [dcl.align]: accepted side of
// over-aligned-element-errors.cpp. `alignas` (and GNU `aligned`) on a
// variable aligns the object, not its type; a class with `aligned` is
// padded to a multiple of its alignment; an `aligned` typedef that keeps
// the size a multiple of the alignment (equal, smaller than the natural
// alignment, or on a pointer typedef only through a further array) is
// fine; a packed class has alignment 1; a template argument is the
// canonical type, so `T[2]` with `T` an over-aligned typedef is accepted
// (as clang and GCC do). Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
typedef int I __attribute__((aligned(16)));
alignas(16) int ok1[2];
int ok1b[2] __attribute__((aligned(16)));
struct __attribute__((aligned(16))) P { int x; };
P ok2[2];
static_assert(sizeof(P) == 16 && sizeof(ok2) == 32);
typedef int Small __attribute__((aligned(2)));
Small ok3[2];
typedef int Same __attribute__((aligned(4)));
Same ok4[2];
typedef I K __attribute__((aligned(4)));
K ok5[2];
struct __attribute__((packed)) Pk { char c; int x; };
Pk ok6[2];
static_assert(sizeof(ok6) == 10);
typedef I *IP;
IP ok7[2];
typedef int I4[4] __attribute__((aligned(16)));
I4 ok8[2];
static_assert(sizeof(ok8) == 32);
struct Q { alignas(16) int m; };
Q ok9[2];
template <class T> struct S { T a[2]; };
S<I> s;
template <class T> void g() { T t[2]; (void)t; }
template void g<I>();
I one;
static_assert(alignof(I) == 16 && sizeof(I) == 4);
