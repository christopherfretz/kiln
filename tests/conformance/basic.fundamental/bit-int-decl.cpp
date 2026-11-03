// [basic.fundamental]/1 (C23 6.2.5, 6.7.2 as a clang extension in C++):
// `_BitInt(N)` takes an integral constant width, at least 2 when signed
// and 1 when unsigned, at most BITINT_MAXWIDTH (8388608 on x86-64);
// `_ExtInt` is its deprecated spelling; an enumeration cannot have a
// `_BitInt` underlying type ([dcl.enum]/2).
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 22:1 error "signed _BitInt must have a bit size of at least 2"
//@ diag: 23:1 error "unsigned _BitInt must have a bit size of at least 1"
//@ diag: 24:1 error "signed _BitInt of bit sizes greater than 8388608 not supported"
//@ diag: 26:1 warning "'_ExtInt' is deprecated; use '_BitInt' instead"
//@ diag: 29:9 error "expression is not an integral constant expression"
//@ diag: 29:9 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 28:5 note "declared here"
//@ diag: 30:1 error "signed _BitInt of bit sizes greater than 8388608 not supported"
//@ diag: 31:9 error "integral constant expression must have integral or unscoped enumeration type, not 'double'"
//@ diag: 33:10 error "'_BitInt(8)' is an invalid underlying type"
//@ diag: 34:20 error "'unsigned _BitInt(8)' is an invalid underlying type"
_BitInt(8) a;
unsigned _BitInt(1) b;
_BitInt(1) c;
unsigned _BitInt(0) d;
_BitInt(8388609) e;
unsigned _BitInt(8388608) e2;
_ExtInt(8) f;
signed _BitInt(4) g;
int n = 3;
_BitInt(n) h;
_BitInt(-3) i;
_BitInt(3.0) j;
_BitInt(8) unsigned l;
enum E : _BitInt(8) { X };
enum E2 : unsigned _BitInt(8) { Y };
static_assert(__BITINT_MAXWIDTH__ == 8388608);
static_assert(sizeof(_BitInt(8)) == 1 && sizeof(_BitInt(9)) == 2);
static_assert(sizeof(_BitInt(33)) == 8 && alignof(_BitInt(33)) == 8);
static_assert(sizeof(_BitInt(65)) == 16 && sizeof(_BitInt(129)) == 24);
