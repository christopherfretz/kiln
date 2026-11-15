// `-fshort-enums` in C (GCC and clang): an enumeration takes the smallest
// type that holds its values, as `__attribute__((packed))` gives one
// (C23 6.7.3.3; the choice of compatible type is implementation-defined).
// WP-1461.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fshort-enums
enum D { d1 = -128, d2 = 127 };
enum E { e1 = 0 };
enum F { f1 = 300 };
_Static_assert(sizeof(enum D) == 1, "");
_Static_assert(sizeof(enum E) == 1, "");
_Static_assert(sizeof(enum F) == 2, "");
_Static_assert(_Generic(d1, int: 1, default: 0), "enumeration constants are int");
