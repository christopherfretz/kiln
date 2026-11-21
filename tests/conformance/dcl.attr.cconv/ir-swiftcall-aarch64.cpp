// [dcl.fct] [dcl.attr.grammar]/5 `swiftcall` lowers as clang on AArch64:
// `swiftcc` with clang's Swift argument lowering (SwiftCallingConv.cpp),
// not AAPCS64, under the AArch64 Swift vector legality (a power-of-2
// element count in 8 or 16 bytes): classes split into byte-range
// entries, at most four registers, else indirect without `byval`;
// vectors split into legal vectors; `long double` as `fp128`.
// spec/abi.md §5.6, WP-1745 (WP-808).
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu
#define SC __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
typedef float v4f __attribute__((vector_size(16)));
typedef float v8f __attribute__((vector_size(32)));
typedef float v2f __attribute__((vector_size(8)));
typedef int v16i __attribute__((vector_size(64)));
typedef char v32c __attribute__((vector_size(32)));
typedef float v3f __attribute__((ext_vector_type(3)));
typedef short v3s __attribute__((ext_vector_type(3)));
struct A { int a; double b; };
struct B { char c[3]; short s; };
struct C { long a, b, c, d, e; };
struct D { float x, y, z; };
struct E { E(const E &); int a; };
struct F { __int128 x; };
struct G { int a[2]; char c; float f; };
struct H { v4f v; };
struct I { char a; long long b; char c; };
struct Em {};
union U { int i; float f; };
union U2 { long l; double d; char c[12]; };
struct BF { int a : 3; int b : 9; char c; };
struct Base { int x; };
struct Der : Base { float y; short z; };
struct Bool { bool a, b; int c; };
struct LD { long double x; };
struct CA { char c[9]; };
struct V3 { v3f v; float w; };
struct V2 { char c; v2f v; };
struct Pk { char c; int i; } __attribute__((packed));
struct At { _Atomic(int) a; char b; };
struct MP { int Base::*p; };
struct MFP { void (Base::*p)(); };
enum En { e1, e2 };
struct Ptrs { void *a; int *b; long c; };
struct Five { float a, b, c, d, e; };
struct Mixed { long a; double b; float c; char d; };
struct Cx { _Complex float c; int i; };
struct Nested { CA a; short s; };
struct Arr2 { double d[2]; };
struct Bi { _BitInt(17) b; char c; };
SC A fa(A x) { return x; }
SC B fb(B x) { return x; }
SC C fc(C x) { return x; }
SC D fd(D x) { return x; }
SC void fe(E x) {}
SC F ff(F x) { return x; }
SC G fg(G x) { return x; }
SC v4f fv(v4f x) { return x; }
SC v8f fv8(v8f x) { return x; }
SC H fh(H x) { return x; }
SC I fi(I x) { return x; }
SC void fem(Em x) {}
SC bool fbool(bool b, char c, short s, float f, long double ld) { return b; }
SC _Complex double fcx(_Complex float a) { return a; }
SC U f1(U x) { return x; }
SC U2 f2(U2 x) { return x; }
SC BF f3(BF x) { return x; }
SC Der f4(Der x) { return x; }
SC Bool f5(Bool x) { return x; }
SC LD f6(LD x) { return x; }
SC CA f7(CA x) { return x; }
SC V3 f8(V3 x) { return x; }
SC V2 f9(V2 x) { return x; }
SC Pk f10(Pk x) { return x; }
SC At f11(At x) { return x; }
SC MP f12(MP x) { return x; }
SC MFP f13(MFP x) { return x; }
SC En f14(En x) { return x; }
SC Ptrs f15(Ptrs x) { return x; }
SC Five f16(Five x) { return x; }
SC Mixed f17(Mixed x) { return x; }
SC Cx f18(Cx x) { return x; }
SC Nested f19(Nested x) { return x; }
SC Arr2 f20(Arr2 x) { return x; }
SC v3f f21(v3f x) { return x; }
SC v2f f22(v2f x) { return x; }
SC v16i f23(v16i x) { return x; }
SC v3s f24(v3s x) { return x; }
SC v32c f25(v32c x) { return x; }
SC Bi f26(Bi x) { return x; }
SC void (Base::*f27(void (Base::*p)()))() { return p; }
SC int Base::*f28(int Base::*p) { return p; }
SC _Complex long double f29(_Complex long double x) { return x; }
SC unsigned __int128 f30(__int128 a, _BitInt(65) b) { return a; }
SA void s5(void *ctx) {}
typedef void (*SP)() SC;
void m(SP) {}
void use(A a, B b, C c, D d, E &e, F f, G g, v4f v, H h, I i, Em em, _Complex float cf,
         U2 u2, BF bf, V3 v3, Cx cx) {
  fa(a); fb(b); fc(c); fd(d); fe(e); ff(f); fg(g); fv(v); fh(h); fi(i); fem(em);
  bool r = fbool(true, 'c', 3, 1.0f, 2.0L);
  _Complex double x = fcx(cf);
  (void)(fv(v) + fv(v)[0]);
  f2(u2); f3(bf); f8(v3); f18(cx);
  A (*pa)(A) SC = fa;
  pa(a);
}
