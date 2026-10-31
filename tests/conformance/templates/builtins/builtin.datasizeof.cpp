// [expr.sizeof] (clang extension __datasizeof): the size of a type without
// its tail padding, which clang computes as the record's data size (the
// Itanium dsize) and as sizeof for every other type. libc++ uses it to
// decide whether a type's tail padding may be reused (<__type_traits/datasizeof.h>).
//@ expect: compile-ok
//@ strict-diags
struct A { int i; char c; };            // POD for layout: dsize == sizeof
struct B { B(); int i; char c; };       // not POD: dsize 5
struct C : B { char d; };               // d reuses B's tail padding
struct E {};
struct F { F(); };
static_assert(__datasizeof(int) == 4);
static_assert(__datasizeof(char[3]) == 3);
static_assert(__datasizeof(A) == 8);
static_assert(__datasizeof(B) == 5);
static_assert(sizeof(B) == 8);
static_assert(__datasizeof(C) == 6);
static_assert(__datasizeof(E) == 1);
static_assert(__datasizeof(F) == 0 && sizeof(F) == 1);
static_assert(__datasizeof(B[2]) == 16);
static_assert(__datasizeof(B&) == 5);
template <class T> constexpr unsigned long ds = __datasizeof(T);
static_assert(ds<B> == 5 && ds<double> == 8);
B b();
constexpr int k = 0;
static_assert(__datasizeof k == 4 && __datasizeof(k) == 4);
