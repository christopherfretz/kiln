// [dcl.fct] [dcl.attr.grammar]/5 A calling-convention attribute is part of
// the function type: the AArch64 conventions (`preserve_most`,
// `preserve_all`, `preserve_none`, `aarch64_vector_pcs`,
// `aarch64_sve_pcs`, `ms_abi`) lower as clang 19 on definitions,
// declarations and every call site, direct, virtual, through a pointer
// and through a member pointer; only `ms_abi` is mangled. spec/types.md
// §5.4, WP-808.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu
#define PM __attribute__((preserve_most))
#define PA __attribute__((preserve_all))
#define PN __attribute__((preserve_none))
#define MS __attribute__((ms_abi))
#define AV __attribute__((aarch64_vector_pcs))
#define AS __attribute__((aarch64_sve_pcs))
PM int pm(int x) { return x + 1; }
PA int pa(int x);
PN int pn(int x, int y) { return x + y; }
AV double av(double a, float b) { return a + b; }
AS int as(int a);
struct V { PM virtual int v(int); PA int m(int) const; static PM int s(int); };
int V::v(int x) { return x; }
int V::s(int x) { return x; }
typedef PM int (*FP)(int);
int over(int (*)(int)) { return 1; }
typedef MS int (*MF)(int);
MS int ms(int x) { return x; }
int over(MF) { return 2; }
int use(V &v, const V &cv, FP f, int (V::*mp)(int) const PA, AV double (*q)(double)) {
  int r = pm(1) + pa(2) + v.v(3) + cv.m(4) + V::s(5) + f(6) + (cv.*mp)(7);
  FP g = pm; r += g(8);
  r += [](int x) PM { return x; }(9);
  FP lp = [](int x) PM { return x; }; r += lp(10);
  r += pn(1, 2) + int(av(1, 2)) + as(3) + int(q(1.0));
  return r + over(ms) + over(+[](int) { return 0; });
}
struct D : V { PM int v(int) override; };
int D::v(int x) { return -x; }
typedef void (*PMP)() PM;
typedef void (*AVP)() AV;
typedef void (*MSP)() MS;
void m2(PMP) {} void m8(MSP) {} void m9(AVP) {}
template<class T> void t(T) {}
template void t<MSP>(MSP);
template void t<AVP>(AVP);
