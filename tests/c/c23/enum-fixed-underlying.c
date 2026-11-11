// C23 6.7.2.2p5-p9: an enumeration with a fixed underlying type; its
// constants have the enumerated type, and an enumeration whose values
// do not fit in int has a larger underlying type.
//@ expect: compile-ok
//@ std: c23
//@ strict-diags
enum E : unsigned char { A, B = 200 };
static_assert(sizeof(enum E) == 1);
static_assert(_Generic(A, enum E: 1, default: 0));
enum Big { L = 0x100000000 };
static_assert(sizeof(enum Big) == 8);
