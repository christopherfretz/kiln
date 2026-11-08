// GNU `ms_struct` and `#pragma ms_struct` (WP-809): sizes, offsets and the
// bytes each bit-field occupies, set to all ones in a zeroed object, under
// the Microsoft bit-field rules (clang `IsMsStruct` layout and the
// discrete bit-field IR, one storage unit per run of same-sized fields).
//@ expect: run
//@ stdout: S1 size 12 align 4
//@ stdout: S1.a:070000000000000000000000
//@ stdout: S1.b:780000000000000000000000
//@ stdout: S1.c:000000001f00000000000000
//@ stdout: S1.d:000000000000000003000000
//@ stdout: S2 size 8 align 4
//@ stdout: S2.a:0700000000000000
//@ stdout: S2.b:0000000003000000
//@ stdout: S3 size 8 align 4
//@ stdout: S3.b at 4
//@ stdout: S4 size 2 align 1
//@ stdout: S4.a:0700
//@ stdout: S4.b:0003
//@ stdout: S5 size 1 align 1
//@ stdout: S5.c at 0
//@ stdout: S6 size 24 align 8
//@ stdout: S6.a:070000000000000000000000000000000000000000000000
//@ stdout: S6.b:780000000000000000000000000000000000000000000000
//@ stdout: S6.c:00000000000000007f000000000000000000000000000000
//@ stdout: S6.d:00000000000000000000000000000000ffffffffffffff0f
//@ stdout: S7 size 6 align 2
//@ stdout: S7.a:ff0300000000
//@ stdout: S7.b:0000ff030000
//@ stdout: S7.c:00000000ff0f
//@ stdout: S8 size 24 align 8
//@ stdout: S8.a at 0
//@ stdout: S8.c at 16
//@ stdout: S8.b:000000000000000007000000000000000000000000000000
//@ stdout: S9 size 16 align 4
//@ stdout: S9.c at 8
//@ stdout: S9.d at 9
//@ stdout: S9.b:000000000f0000000000000000000000
//@ stdout: S10 size 16 align 4
//@ stdout: S10.c at 8
//@ stdout: S10.d at 9
//@ stdout: S10.b:000000000f0000000000000000000000
//@ stdout: S11 size 8 align 1
//@ stdout: S11.d at 7
//@ stdout: S11.b:000f000000000000
//@ stdout: S11.c:0000000000070000
//@ stdout: S12 size 16 align 2
//@ stdout: S12.d at 14
//@ stdout: S12.b:00000f00000000000000000000000000
//@ stdout: S12.c:00000000000007000000000000000000
//@ stdout: S13 size 32 align 16
//@ stdout: S13.c at 20
//@ stdout: S13.a:0300000000000000000000000000000000000000000000000000000000000000
//@ stdout: S13.b:000000000000000000000000000000000f000000000000000000000000000000
//@ stdout: U1 size 4 align 1
//@ stdout: U1.a:07000000
//@ stdout: U1.b:ff010000
//@ stdout: U1.c:01000000
//@ stdout: U2 size 1 align 1
//@ stdout: U2.b:03
//@ stdout: U3 size 4 align 1
//@ stdout: U3.b:0f000000
//@ stdout: S14 size 6 align 2
//@ stdout: S14.a:010000000000
//@ stdout: S14.b:020000000000
//@ stdout: S14.e:000003000000
//@ stdout: S14.s:00001c000000
//@ stdout: S14.c:000000000100
//@ stdout: S15 size 8 align 4
//@ stdout: S16 size 3 align 1
//@ stdout: S16.b:000f00
//@ stdout: S16.c:00f000
//@ stdout: S16.d:000001
//@ stdout: S17 size 2 align 1
//@ stdout: S17.a:0f00
//@ stdout: S17.b:f000
//@ stdout: S17.c:0001
//@ stdout: S18 size 8 align 4
//@ stdout: S18.a:ffffff7f00000000
//@ stdout: S18.b:0000000003000000
//@ stdout: S18.c:0000000004000000
//@ stdout: S19 size 24 align 8
//@ stdout: S19.a:010000000000000000000000000000000000000000000000
//@ stdout: S19.b:000000000000000001000000000000000000000000000000
//@ stdout: S19.c:000000000000000000000000000000000100000000000000
//@ stdout: S20 size 12 align 4
//@ stdout: S20.a:010000000000000000000000
//@ stdout: S20.b:000000000700000000000000
//@ stdout: S20.c:000000000000000003000000
//@ stdout: P1 size 8 align 4
//@ stdout: P1.b:000000000f000000
//@ stdout: P2 size 4 align 1
//@ stdout: P2.b:0f000000
//@ stdout: P3 size 4 align 4
//@ stdout: P3.b:78000000
//@ stdout: P4 size 4 align 4
//@ stdout: P4.b:78000000
//@ stdout: T1<int> size 8 align 4
//@ stdout: T1<int>.b:000000000f000000
//@ stdout: T1<char> size 1 align 1
//@ stdout: T1<char>.b:78
//@ stdout: S21 size 8 align 4
//@ stdout: S21.in at 4
//@ stdout: S21.d:0000000000070000
//@ stdout: S22 size 16 align 4
//@ stdout: S22.x at 4
//@ stdout: S22.b:00000000000000000000000007000000
//@ stdout: S23 size 8 align 4
//@ stdout: S23.a:0700000000000000
//@ stdout: S23.b:f800000000000000
//@ stdout: S23.c:0000000007000000
//@ stdout: vals 1 2 3 1
//@ stdout: neg -7
#pragma clang diagnostic ignored "-Wbitfield-constant-conversion"
#include <cstdio>
#include <cstddef>
#include <cstring>

#define MS __attribute__((ms_struct))

template <class T> void dump(const char *n, const T &t) {
  const unsigned char *p = reinterpret_cast<const unsigned char *>(&t);
  std::printf("%s:", n);
  for (std::size_t i = 0; i < sizeof(T); ++i) std::printf("%02x", p[i]);
  std::printf("\n");
}
// Sets bit-field F of a zeroed T to all ones and prints the bytes.
#define BF(T, F) do { T t; std::memset(&t, 0, sizeof t); t.F = decltype(t.F)(~0ull); dump(#T "." #F, t); \
  if (!(t.F + 1 == 0 || t.F != 0)) std::printf("bad\n"); } while (0)
#define SZ(T) std::printf(#T " size %zu align %zu\n", sizeof(T), alignof(T))
#define OF(T, F) std::printf(#T "." #F " at %zu\n", offsetof(T, F))

struct MS S1 { char a : 3; char b : 4; int c : 5; char d : 2; };
struct MS S2 { char a : 3; int : 0; char b : 2; };
struct MS S3 { int a; int : 0; char b; };
struct MS S4 { char a : 3; char : 0; char b : 2; };
struct MS S5 { int : 0; char c; };
struct MS S6 { int a : 3; unsigned b : 4; long long c : 7; unsigned long long d : 60; };
struct MS S7 { short a : 10; short b : 10; short c : 12; };
struct MS S8 { char a; long long b : 3; char c; };
struct __attribute__((packed, ms_struct)) S9 { char a; int b : 4; char c; int d; };
struct MS S10 { char a; int b : 4 __attribute__((packed)); char c; int d __attribute__((packed)); };
#pragma pack(push, 1)
struct MS S11 { char a; int b : 4; short c : 3; char d; };
#pragma pack(2)
struct MS S12 { char a; int b : 4; long long c : 3; char d; };
#pragma pack(pop)
struct MS S13 { char a : 2; int b : 4 __attribute__((aligned(16))); char c; };
union MS U1 { char a : 3; int b : 9; short c : 1; };
union MS U2 { char : 0; char b : 2; };
union MS U3 { int b : 4 __attribute__((aligned(8))); char c; };
enum E : short { e0, e1 };
struct MS S14 { bool a : 1; bool b : 1; E e : 2; short s : 3; bool c : 1; };
struct MS S15 { char a; int : 4; };
struct MS S16 { char a; char b : 4; char c : 4; char d : 1; };
struct MS S17 { char a : 4; unsigned char b : 4; signed char c : 1; };
struct MS S18 { int a : 31; int b : 2; int c : 1; };
struct MS S19 { long long a : 1; int b : 1; long long c : 1; };
struct MS S20 { char a : 1; char : 0; char : 0; int b : 3; char : 0; int c : 2; };
#pragma ms_struct on
struct P1 { char a : 3; int b : 4; };
union P2 { char a : 3; int b : 4; };
#pragma ms_struct off
struct P3 { char a : 3; int b : 4; };
#pragma ms_struct on
#pragma ms_struct reset
struct P4 { char a : 3; int b : 4; };
template <class T> struct MS T1 { char a : 3; T b : 4; };
struct MS S21 { int a : 4; struct In { char c : 2; } in; char d : 3; };
struct MS S22 { char a : 2; int x[2]; char b : 3; };
struct MS S23 { _BitInt(17) a : 3; _BitInt(17) b : 5; char c : 3; };

int main() {
  SZ(S1); BF(S1, a); BF(S1, b); BF(S1, c); BF(S1, d);
  SZ(S2); BF(S2, a); BF(S2, b);
  SZ(S3); OF(S3, b);
  SZ(S4); BF(S4, a); BF(S4, b);
  SZ(S5); OF(S5, c);
  SZ(S6); BF(S6, a); BF(S6, b); BF(S6, c); BF(S6, d);
  SZ(S7); BF(S7, a); BF(S7, b); BF(S7, c);
  SZ(S8); OF(S8, a); OF(S8, c); BF(S8, b);
  SZ(S9); OF(S9, c); OF(S9, d); BF(S9, b);
  SZ(S10); OF(S10, c); OF(S10, d); BF(S10, b);
  SZ(S11); OF(S11, d); BF(S11, b); BF(S11, c);
  SZ(S12); OF(S12, d); BF(S12, b); BF(S12, c);
  SZ(S13); OF(S13, c); BF(S13, a); BF(S13, b);
  SZ(U1); BF(U1, a); BF(U1, b); BF(U1, c);
  SZ(U2); BF(U2, b);
  SZ(U3); BF(U3, b);
  SZ(S14); BF(S14, a); BF(S14, b); BF(S14, e); BF(S14, s); BF(S14, c);
  SZ(S15);
  SZ(S16); BF(S16, b); BF(S16, c); BF(S16, d);
  SZ(S17); BF(S17, a); BF(S17, b); BF(S17, c);
  SZ(S18); BF(S18, a); BF(S18, b); BF(S18, c);
  SZ(S19); BF(S19, a); BF(S19, b); BF(S19, c);
  SZ(S20); BF(S20, a); BF(S20, b); BF(S20, c);
  SZ(P1); BF(P1, b); SZ(P2); BF(P2, b); SZ(P3); BF(P3, b); SZ(P4); BF(P4, b);
  SZ(T1<int>); BF(T1<int>, b); SZ(T1<char>); BF(T1<char>, b);
  SZ(S21); OF(S21, in); BF(S21, d);
  SZ(S22); OF(S22, x); BF(S22, b);
  SZ(S23); BF(S23, a); BF(S23, b); BF(S23, c);
  S1 s{1, 2, 3, 1}; std::printf("vals %d %d %d %d\n", s.a, s.b, s.c, s.d);
  s.c = -7; std::printf("neg %d\n", s.c);
}
