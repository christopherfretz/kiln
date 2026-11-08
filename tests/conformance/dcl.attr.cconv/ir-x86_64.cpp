// [dcl.fct] [dcl.attr.grammar]/5 A calling-convention attribute is part of
// the function type (clang `FunctionType::ExtInfo`): the x86-64
// conventions lower as clang 19 on definitions, declarations and every
// call site, direct, virtual, through a pointer and through a member
// pointer; a lambda's operator() and its pointer conversion keep the
// convention; `ms_abi`, `swiftcall` mangle as `U6ms_abi`, the other
// conventions do not; `regcall` names get the `__regcall3__` prefix;
// `no_caller_saved_registers` and `regparm(N)` are type properties too
// (the first a "no_caller_saved_registers" function attribute, the second
// no codegen on x86-64). spec/types.md §5.4, WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define PM __attribute__((preserve_most))
#define PA __attribute__((preserve_all))
#define PN __attribute__((preserve_none))
#define MS __attribute__((ms_abi))
#define RC __attribute__((regcall))
#define VC __attribute__((vectorcall))
#define IO __attribute__((intel_ocl_bicc))
#define NC __attribute__((no_caller_saved_registers))
PM int pm(int x) { return x + 1; }
PA int pa(int x);
PN int pn(int x, int y) { return x + y; }
VC double vc(double a, float b) { return a + b; }
IO int io(int a) { return a; }
NC void nc() {}
struct V { PM virtual int v(int); PA int m(int) const; static PM int s(int); };
int V::v(int x) { return x; }
int V::s(int x) { return x; }
typedef PM int (*FP)(int);
int over(int (*)(int)) { return 1; }
typedef MS int (*MF)(int);
MS int ms(int x) { return x; }
int over(MF) { return 2; }
int use(V &v, const V &cv, FP f, int (V::*mp)(int) const PA, NC void (*q)()) {
  int r = pm(1) + pa(2) + v.v(3) + cv.m(4) + V::s(5) + f(6) + (cv.*mp)(7);
  FP g = pm; r += g(8);
  r += [](int x) PM { return x; }(9);
  FP lp = [](int x) PM { return x; }; r += lp(10);
  r += pn(1, 2) + int(vc(1, 2)) + io(3);
  q(); nc();
  return r + over(ms) + over(+[](int) { return 0; });
}
struct D : V { PM int v(int) override; };
int D::v(int x) { return -x; }
// Mangling: only ms_abi is mangled (among the x86-64 conventions).
typedef void (*NCP)() NC;
typedef void (*PMP)() PM;
typedef void (*IOP)() IO;
typedef void (*VCP)() VC;
typedef void (*RCP)() RC;
typedef void (*MSP)() MS;
void m1(NCP) {} void m2(PMP) {} void m3(IOP) {} void m4(VCP) {} void m5(RCP) {} void m8(MSP) {}
template<class T> void t(T) {}
template void t<MSP>(MSP);
template void t<NCP>(NCP);
// regcall names.
extern "C" RC int cf(int x) { return x; }
template<class T> RC T tf(T x) { return x; }
int u = tf(1);
namespace N { RC void nf() {} }
struct S { RC S(); RC int operator+(int); };
S::S() {}
int S::operator+(int) { return 0; }
// regparm: part of the type, no x86-64 codegen.
__attribute__((regparm(2))) int f2(int a, int b) { return a + b; }
typedef int (*RP)(int, int) __attribute__((regparm(2)));
void take(RP) {}
void take(int (*)(int, int));
int g2() { take(f2); RP p = f2; return p(1, 2) + cf(1) + (S() + 1); }
