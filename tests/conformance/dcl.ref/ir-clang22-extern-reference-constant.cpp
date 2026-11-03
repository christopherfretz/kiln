// [dcl.ref]/1 (a reference is never rebound), [dcl.type.cv]/4: an
// external declaration of a reference or of a `const` non-class object
// is `external constant` (clang `GetOrCreateLLVMGlobal` /
// `isConstantStorage`); a `const` class object, which may be constructed
// at run time, stays `external global`. Compared with clang 22's IR exactly.
//@ expect: ir
//@ requires: clang>=22
struct P { int a; };
struct M { mutable int a; };
extern int &er;
extern const int &cer;
extern int &&rer;
extern const int ci;
extern const volatile int cvi;
extern const P cp;
extern const M cm;
extern const int ca[2];
extern int *const cptr;
extern const P cpa[2];
extern int (&efr)(int);
extern int plain;
int use() {
  return er + cer + rer + ci + cvi + cp.a + cm.a + ca[1] + *cptr + cpa[0].a
       + efr(1) + plain;
}
