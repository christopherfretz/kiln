// [dcl.fct] [dcl.attr.grammar]/5 An `ms_abi` function on x86_64 Linux
// uses the Win64 argument classification as clang 19
// (`WinX86_64ABIInfo` / `AArch64ABIInfo` with the Win64 variant): records
// of 1, 2, 4 or 8 bytes in an integer, larger ones and non-trivial ones
// indirectly, `long double`, `__int128`, vectors, complex and
// `_BitInt`, member pointers, `this` and a variadic call with an HFA
// (no HFA in a Win64 variadic call on AArch64). spec/types.md §5.4,
// WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define MS __attribute__((ms_abi))
struct S1 { char c; }; struct S2 { short a; }; struct S3 { char a[3]; }; struct S4 { int a; };
struct S8 { int a, b; }; struct S12 { int a, b, c; }; struct S16 { long a, b; };
struct F2 { float a, b; }; struct D1 { double d; }; struct D2 { double a, b; };
struct NT { int a; ~NT(); }; struct E {}; enum En : short { A }; enum class Eb : unsigned char { B };
struct Base { int x; }; struct Der : Base { int y; };
typedef float v4f __attribute__((vector_size(16)));
typedef double v2d __attribute__((vector_size(16)));
typedef float v8f __attribute__((vector_size(32)));
MS S1 f1(S1 a) { return a; }
MS S2 f2(S2 a) { return a; }
MS S3 f3(S3 a) { return a; }
MS S4 f4(S4 a) { return a; }
MS S8 f8(S8 a) { return a; }
MS S12 f12(S12 a) { return a; }
MS S16 f16(S16 a) { return a; }
MS F2 ff2(F2 a) { return a; }
MS D1 fd1(D1 a) { return a; }
MS D2 fd2(D2 a) { return a; }
MS NT fnt(NT a) { return NT{1}; }
MS E fe(E a) { return a; }
MS En fen(En a, Eb b) { return a; }
MS bool fb(bool a, char c, short s, unsigned short us) { return a; }
MS __int128 fi(__int128 a, unsigned __int128 b) { return a; }
MS long double fld(long double a) { return a; }
MS double fdd(float a, double b) { return b; }
MS v4f fv(v4f a, v2d b) { return a; }
MS v8f fv8(v8f a) { return a; }
MS _Complex float fcf(_Complex float a) { return a; }
MS _Complex double fcd(_Complex double a) { return a; }
MS Der fder(Der a) { return a; }
MS int (Base::*fmp(int (Base::*p)()))() { return p; }
MS int Base::* fdm(int Base::* p) { return p; }
MS _BitInt(48) fbi(_BitInt(48) a, _BitInt(65) b, _BitInt(128) c) { return a; }
MS void fvar(int n, ...) {}
MS void fn(decltype(nullptr) p, int *q, void (*fp)()) {}
struct M { MS int m(S16); MS S16 r(int); MS virtual S12 v(S12); };
int M::m(S16) { return 0; }
S16 M::r(int) { return {}; }
S12 M::v(S12 a) { return a; }
int call(M &m) { S16 s{}; S12 t{}; fvar(1, 2.0, s, t); return m.m(s) + m.r(1).a + m.v(t).a + f12(t).a + fd2({1,2}).a; }
struct H { float a, b, c; };
struct H4 { double a, b, c, d; };
#define MV __attribute__((ms_abi))
MV void f(H, ...) {}
MV void g(H) {}
MV H h(int, ...) { return {}; }
void k(H x, H4 y) { f(x, x, y); g(x); }
