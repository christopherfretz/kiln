// C23 6.2.5 / [basic.fundamental] (a clang extension in C++, spec/types.md
// §5.3): `_BitInt(N)` lowers to `iN` in registers, its ABI-sized slot in
// memory (loads/stores of the `iN` value; clang widens in-memory values
// to the slot), with no integer promotion. Widths 7, 33 and 128 on both
// targets; 200 where BITINT_MAXWIDTH allows it (x86_64).
using s7 = _BitInt(7);
using u7 = unsigned _BitInt(7);
using s33 = _BitInt(33);
using u33 = unsigned _BitInt(33);
using s128 = _BitInt(128);
using u128 = unsigned _BitInt(128);

s7 add7(s7 a, s7 b) { return a + b; }
u7 mul7(u7 a, u7 b) { return a * b; }
s7 div7(s7 a, s7 b) { return a / b; }
u7 shl7(u7 a, int n) { return a << n; }
s33 sub33(s33 a, s33 b) { return a - b; }
u33 shr33(u33 a, u33 n) { return a >> n; }
s33 neg33(s33 a) { return -a; }
bool lt33(s33 a, s33 b) { return a < b; }
int to_int(s33 a) { return a; }
s33 from_long(long v) { return v; }
double to_double(u33 a) { return a; }
s128 mul128(s128 a, s128 b) { return a * b; }
u128 xor128(u128 a, u128 b) { return a ^ b; }
s7 g7 = 5;
u33 g33 = 1;
void inc() { ++g7; g33 += 3; }

#if __BITINT_MAXWIDTH__ >= 200
using s200 = _BitInt(200);
using u200 = unsigned _BitInt(200);
s200 add200(s200 a, s200 b) { return a + b; }
u200 shl200(u200 a, unsigned n) { return a << n; }
bool eq200(u200 a, u200 b) { return a == b; }
#endif
