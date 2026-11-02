// [basic.fundamental]/12, IEEE 754 NaN propagation: clang's IR builder folds
// constant floating arithmetic and conversions with LLVM's APFloat, NaN
// results included, on every format (half, x87, fp128) and on both triples:
// a NaN operand is the quieted result with its sign and payload (the left
// one first, signaling or not; fsub does not flip it), an invalid operation
// gives the default positive quiet NaN, a NaN conversion shifts the payload
// by the precision difference and quiets it, and a NaN, infinite or
// out-of-range fptosi/fptoui is poison.
#define H(x) ((_Float16)__builtin_nanf(#x))
#define HS(x) ((_Float16)__builtin_nansf(#x))
#define INF ((_Float16)__builtin_inff())
_Float16 c1() { return H(0x2000); }
_Float16 c2() { return HS(0x2000); }
_Float16 c3() { return HS(1); }
_Float16 c4() { return -H(0x4000); }
float c5() { return (float)H(0x2000); }
double c6() { return (double)-H(0x4000); }
_Float16 a1() { return INF - INF; }
_Float16 a2() { return H(0x2a000) + (_Float16)1.0f; }
_Float16 a3() { return -H(0x2a000) * (_Float16)2.0f; }
_Float16 a4() { return (_Float16)1.0f - -H(0x4000); }
_Float16 a5() { return H(0x2000) / -HS(0x4000); }
_Float16 a6() { return HS(0x6000) + H(0x4000); }
_Float16 a7() { return (_Float16)0.0f * INF; }
_Float16 a8() { return (_Float16)0.0f / (_Float16)0.0f; }
long double l1() { return __builtin_infl() - __builtin_infl(); }
long double l2() { return __builtin_nanl("0x15") * 2.0L; }
long double l3() { return -__builtin_nanl("7") + __builtin_nansl("3"); }
long double l4() { return __builtin_nansl("3") / 2.0L; }
long double l5() { return 2.0L - -__builtin_nanl("9"); }
long double l6() { return (long double)__builtin_nanf("0x15"); }
long double l7() { return (long double)-__builtin_nansf("0x15"); }
float l8() { return (float)__builtin_nanl("0x123456789"); }
_Float16 l9() { return (_Float16)__builtin_nansl("0x123456789"); }
int i1() { return (int)H(1); }
int i2() { return (int)INF; }
int i3() { return (int)__builtin_nanf(""); }
unsigned i4() { return (unsigned)-1.5; }
signed char i5() { return (signed char)(_Float16)200.0f; }
int i6() { return (int)(_Float16)-3.75f; }
unsigned i7() { return (unsigned)-0.5; }
