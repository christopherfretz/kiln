// [class.union]/2, [expr.const]/5.10: a bit-field of the active member of a
// union (here nested inside a struct member, as in libc++'s short-string
// representation) may be initialized, read and written; the union's
// active-member check reaches through the struct to the bit-field's unit.
//@ expect: compile-ok
struct Sh { char data[15]; unsigned char size : 7; unsigned char is_long : 1; };
struct L { unsigned long d; unsigned long is_long : 1; };
union U { Sh s; L l; };
constexpr bool h1() { U u{}; return !u.s.data[0] && !u.s.is_long; }
static_assert(h1());
constexpr bool h2() { U u{}; u.s.size = 5; return u.s.size == 5 && !u.s.is_long; }
static_assert(h2());
struct A { U u; constexpr A() : u() {} };
constexpr bool h3() { A a; return !a.u.s.is_long && !a.u.s.size; }
static_assert(h3());
constexpr bool h4() { U u{}; u.l = L{1, 1}; return u.l.is_long == 1; }
static_assert(h4());
struct Sh3 { unsigned char size : 7; };
union U4 { Sh3 s; int l; };
constexpr bool h5() { U4 u{}; return !u.s.size; }
static_assert(h5());
