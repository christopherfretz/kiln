// Signatures only: how every by-value class size and member mix is
// passed and returned (declarations plus address-taking keep them).
struct S1 { char c; };
struct S2 { short s; };
struct S3 { char a, b, c; };
struct S8 { int a, b; };
struct S9 { long a; char b; };
struct S12 { int a, b, c; };
struct S16 { long a, b; };
struct S17 { long a, b; char c; };
struct S32 { long v[4]; };
struct IF { int i; float f; };
struct DI { double d; int i; };
struct F3 { float x, y, z; };
struct D2 { double x, y; };
struct D4 { double a, b, c, d; };
struct LDS { long double v; };
struct PI { const char* s; int n; };
struct E {};
S1 r1(S1); S2 r2(S2); S3 r3(S3); S8 r8(S8); S9 r9(S9); S12 r12(S12);
S16 r16(S16); S17 r17(S17); S32 r32(S32); IF rif(IF); DI rdi(DI);
F3 rf3(F3); D2 rd2(D2); D4 rd4(D4); LDS rlds(LDS); PI rpi(PI); E re(E);
long double rld(long double, int, long double);
__int128 ri128(__int128, int);
void many(long, long, long, long, long, S16, S16, double, D2);
void* table[] = {
  (void*)r1, (void*)r2, (void*)r3, (void*)r8, (void*)r9, (void*)r12,
  (void*)r16, (void*)r17, (void*)r32, (void*)rif, (void*)rdi, (void*)rf3,
  (void*)rd2, (void*)rd4, (void*)rlds, (void*)rpi, (void*)re, (void*)rld,
  (void*)ri128, (void*)many,
};
