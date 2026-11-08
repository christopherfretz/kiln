// [conv.fpint], [expr.cond]: -Wimplicit-int-float-conversion warns when an
// integer converted to a floating type may need more bits than the
// floating type's significand; the bits are the likely range of the
// expression (clang's GetExprRange), not only of its type. A constant
// whose value changes is reported with both values instead
// (-Wimplicit-const-int-float-conversion). Braced initializers, explicit
// casts and constant-evaluated contexts do not warn; each arm of a
// conditional is checked on its own; an instantiation warns again.
//@ verify
//@ args: -Wimplicit-int-float-conversion

int gi; unsigned gu; short gs; long long gll; unsigned long long gull;
struct BF { int b : 20; long long c : 40; } bf;
enum E { EA, EB };
float f1 = gi; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
double d1 = gll; // expected-warning {{implicit conversion from 'long long' to 'double' may lose precision}}
double d2 = gi;
float f2 = gs;
float f3 = gu; // expected-warning {{implicit conversion from 'unsigned int' to 'float' may lose precision}}
double d3 = gull; // expected-warning {{implicit conversion from 'unsigned long long' to 'double' may lose precision}}
float f4 = 16777216;
float f5 = 16777217; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
float f6 = -16777217; // expected-warning {{implicit conversion from 'int' to 'float' changes value from -16777217 to -16777216}}
float f7 = 123456789; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 123456789 to 123456792}}
float f8 = 100000001; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 100000001 to 1.0E+8}}
float f9 = 0x7fffffff; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 2147483647 to 2147483648}}
double d4 = 9223372036854775807LL; // expected-warning {{implicit conversion from 'long long' to 'double' changes value from 9223372036854775807 to 9223372036854775808}}
double d5 = 18446744073709551615ULL; // expected-warning {{implicit conversion from 'unsigned long long' to 'double' changes value from 18446744073709551615 to 18446744073709551616}}
float f10 = gi & 0xffff;
float f11 = gi >> 10;
float f12 = gi >> 7; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
float f13 = gi % 1000;
float f14 = gi / 256;
float f15 = gi / 255; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
float f16 = gi + 1; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
float f17 = bf.b;
double d6 = bf.c;
float f18 = EB;
float f19 = (float)gi;
float f20{16777216};
float f21 = 1 << gi; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
constexpr float cf = 16777217;

void take(float);
void h(bool b, int i, long long ll, unsigned u) {
  take(i); // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  take(16777217); // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
  float a = i * 2.0f; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  bool c = i < 1.5f; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  float x = 0;
  x += i; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  x = i; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  x = ll; // expected-warning {{implicit conversion from 'long long' to 'float' may lose precision}}
  float y = b ? i : 16777217; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}} expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
  float z = i ?: 5; // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
  float w = u >> 8;
  float m[] = {1.0f, (float)i, 16777216};
  (void)sizeof(take(16777217), 0);
  if (false)
    take(16777217);
  (void)a; (void)c; (void)y; (void)z; (void)w; (void)m;
}

constexpr float cfn() { return 16777217; } // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
consteval float cev() { return 16777217; }
float u1 = cfn() + cev();

template <class T> float tg(T, int i) { return i; } // expected-warning {{implicit conversion from 'int' to 'float' may lose precision}} expected-warning {{implicit conversion from 'int' to 'float' may lose precision}} expected-warning {{implicit conversion from 'int' to 'float' may lose precision}}
template float tg(char, int); // expected-note {{in instantiation of function template specialization 'tg<char>' requested here}}
template float tg(short, int); // expected-note {{in instantiation of function template specialization 'tg<short>' requested here}}
template <int N> float tn() { return N; } // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
float u2 = tn<16777217>() + tn<3>(); // expected-note {{in instantiation of function template specialization 'tn<16777217>' requested here}}
