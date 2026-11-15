// C23 6.7.2.2p15: once an enumeration without a fixed underlying type is
// complete, its constants are `int` only if every value fits in `int`;
// otherwise each constant has the enumerated type (clang 22 and GCC apply
// this in every C mode). Found running GCC's C++ testsuite. WP-1461.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
enum L { l1 = -1, l2 = 0x80000000 };
enum U { u1 = 1, u2 = 0x80000000 };
enum I { i1 = -1, i2 = 7 };
enum __attribute__((packed)) P { p1 = 1, p2 = 200 };
_Static_assert(sizeof(l1) == sizeof(long), "every L constant is long");
_Static_assert(_Generic(l1, long: 1, default: 0), "");
_Static_assert(_Generic(u1, unsigned: 1, default: 0), "every U constant is unsigned");
_Static_assert(_Generic(u2, unsigned: 1, default: 0), "");
_Static_assert(_Generic(i1, int: 1, default: 0), "I fits in int");
_Static_assert(_Generic(p2, int: 1, default: 0), "P fits in int");
