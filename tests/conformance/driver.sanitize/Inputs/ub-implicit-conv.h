// Conversions for the implicit-integer-* and implicit-bitfield-conversion
// checks (clang EmitIntegerTruncationCheck, EmitIntegerSignChangeCheck,
// EmitBitfieldConversionCheck; WP-871).
namespace n1 {
typedef unsigned char u8;
enum E { A, B = 300 };
unsigned char t1(int x) { return x; }
signed char t2(int x) { return x; }
int t3(unsigned x) { return x; }
unsigned t4(int x) { return x; }
short t5(unsigned long x) { return x; }
unsigned short t6(long x) { return x; }
long t7(unsigned x) { return x; }
unsigned long t8(int x) { return x; }
u8 t9(E e) { return e; }
void t10(short &s, int v) { s += v; }
void t11(unsigned char &c) { c *= 3; }
void t12(char &c) { ++c; c--; }
void t13(unsigned short &c) { c++; --c; }
void t14(char32_t &c, wchar_t &w, char16_t &h) { c++; w++; h--; }
int t15(long x) { return (int)x; }
int t16(long x) { return static_cast<int>(x + 1); }
bool t17(int x) { return x; }
struct S { int b : 3; unsigned c : 4; };
void t18(S &s, int v) { s.b = v; s.c += v; s.b++; }
void t19(unsigned char *p, int n) { p[0] = n; }
int t20(char c) { return c; }
unsigned t21(char c) { return c; }
long long t22(unsigned long long x) { return x; }
} // namespace n1
namespace n2 {
enum E { A, B };
struct S { int b : 3; unsigned c : 4; signed char d : 2; unsigned long long e : 40; long f : 33; E g : 2; bool h : 1; char i : 8; unsigned short j : 16; };
void t1(S &s, int v) { s.b = v; }
void t2(S &s, unsigned v) { s.b = v; s.c = v; }
void t3(S &s, long v) { s.c = v; s.e = v; s.f = v; }
void t4(S &s, int v) { s.c += v; s.d -= v; s.b *= v; }
void t5(S &s) { s.b++; --s.c; s.d++; s.i++; s.j--; s.e++; }
void t6(S &s, signed char v) { s.d = v; s.e = v; }
void t7(S &s, int v) { s.g = (E)v; s.h = v; }
int t8(S &s, int v) { return s.b = v; }
void t9(S &s, S &o) { s.b = o.c; s.c = o.b; s.b = (o.d); }
void t10(S &s, short v) { s.i = v; s.j = v; s.f = v; }
void t11(S &s, int v) { (s.b) = v; s.b = s.c = v; }
} // namespace n2
