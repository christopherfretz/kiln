// Debug info ([dcl.fct]; clang `getDwarfCC`): a function type with a
// non-default calling convention carries it as `cc:` on its
// DISubroutineType (DW_AT_calling_convention, which LLVM also writes on
// the subprogram's DIE): definitions, member functions, declarations
// named by a using-declaration, and function pointer types. A convention
// the target defaults or ignores (`sysv_abi` on x86-64 Linux) has none.
//@ args: -g -Wno-ignored-attributes
#define CC(c) __attribute__((c))
void CC(preserve_most) pm(int) {}
void CC(preserve_all) pa(int) {}
void CC(preserve_none) pn(int) {}
void (CC(preserve_most) *fp)(int) = pm;
typedef void CC(preserve_all) PAFn(int);
PAFn *pafp = pa;
struct S {
  void CC(preserve_most) m(int) const;
  static void CC(preserve_all) s();
};
void S::m(int) const {}
void S::s() {}
namespace N { void CC(preserve_most) d(int); }
using N::d;
#ifdef __x86_64__
void CC(ms_abi) ms(int) {}
void CC(sysv_abi) sv(int) {}
void CC(vectorcall) vc(int) {}
void CC(regcall) rc(int) {}
void CC(intel_ocl_bicc) io(int) {}
#endif
#ifdef __aarch64__
void CC(aarch64_vector_pcs) vp(int) {}
void CC(aarch64_sve_pcs) sp(int) {}
#endif
void call() { d(1); }
