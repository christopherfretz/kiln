// [basic.extended.fp], [expr.arith.conv], [conv.double], [conv.fpint]
// (C++23, P1467R9): run-time arithmetic, conversions and argument passing
// for _Float16, __bf16, _Float32, _Float64 and _Float128. The inputs are
// volatile so nothing folds; every operation is stored before the next one
// so excess precision cannot show. The expected output is GCC 14's
// (g++ -std=c++23, x86-64 and AArch64).
//@ expect: run
//@ std: c++23
//@ oracle: none
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no _Float32/_Float64, no bf16/f128 suffixes)
//@ stdout: bf16 0x1p+0 0x1.04p+0 0x1.56p-2 0x1.9ap-4 300 304
//@ stdout: f16 0x1p+11 0x1.554p-2 0x1.998p-4
//@ stdout: f32 0x1.555556p-2 0x1p+24 f64 0x1.5555555555555p-2
//@ stdout: f128 0x1.5555555555555p-2 1 0 -7 18446744073709551615
//@ stdout: mixed 0x1.8p+1 0x1.8p+1 0x1.4p+1 0x1.8p+1
//@ stdout: args 0x1.8p+0 0x1.8p+1 0x1.2p+2 0x1.8p+2 0x1.ep+2
extern "C" int printf(const char *, ...);
volatile int one = 1, three = 3;
volatile double tenth = 0.1;
volatile unsigned long long big = -1ULL;

template<class T> T add(T a, T b) { T r = a + b; return r; }
template<class T> T div(T a, T b) { T r = a / b; return r; }

__bf16 pb(__bf16 x) { return x; }
_Float16 ph(_Float16 x) { return x * (_Float16)2; }
_Float32 p32(_Float32 x, __bf16 y) { return x + (_Float32)y * 2; }
_Float64 p64(_Float16 a, _Float32 b, _Float64 c) { return (_Float64)a + (_Float64)b + c; }
_Float128 p128(_Float128 x, int n) { return x * n; }
struct S { __bf16 b; _Float16 h; _Float32 f; };
double ps(S s) { return (double)s.b + (double)s.h + (double)s.f; }

int main() {
  __bf16 b1 = (__bf16)one, b3 = (__bf16)three;
  __bf16 r1 = add(b1, (__bf16)0x1p-8f);          // tie, to even
  __bf16 r2 = add((__bf16)0x1.02p0f, (__bf16)0x1p-8f);
  __bf16 r3 = div(b1, b3);
  __bf16 r4 = (__bf16)tenth;
  printf("bf16 %a %a %a %a %d %d\n", (double)r1, (double)r2, (double)r3, (double)r4,
         (int)(__bf16)(301 * one), (int)(__bf16)(303 * one));
  _Float16 h1 = (_Float16)one;
  printf("f16 %a %a %a\n", (double)add((_Float16)2048, h1), (double)div(h1, (_Float16)three),
         (double)(_Float16)tenth);
  _Float32 f1 = (_Float32)one;
  printf("f32 %a %a f64 %a\n", (double)div(f1, (_Float32)three),
         (double)add((_Float32)16777216, f1), (double)div((_Float64)one, (_Float64)three));
  _Float128 q1 = (_Float128)one, q3 = (_Float128)three;
  _Float128 third = div(q1, q3);
  _Float128 sum = add(q1, (_Float128)0x1p-100 * one);
  printf("f128 %a %d %d %d %llu\n", (double)third, sum != q1, add(q1, (_Float128)0x1p-113 * one) != q1,
         (int)(_Float128)(-7.5 * one), (unsigned long long)(_Float128)big);
  _Float32 m1 = (_Float32)one + 2.0f;          // _Float32
  _Float64 m2 = (_Float64)one + 2.0;           // _Float64
  float m3 = b1 + 1.5f;                        // float
  _Float64 m4 = (_Float32)one + 2.0f64;        // _Float64
  printf("mixed %a %a %a %a\n", (double)m1, (double)m2, (double)m3, (double)m4);
  S s{(__bf16)1.5f, (_Float16)2.5f, (_Float32)3.5f};
  printf("args %a %a %a %a %a\n", (double)pb((__bf16)1.5f), (double)ph((_Float16)1.5f),
         (double)p32((_Float32)one * 1.5f32, (__bf16)1.5f), (double)p64(1.5f16, 2.0f32, 2.5f64) + 0,
         ps(s));
}
