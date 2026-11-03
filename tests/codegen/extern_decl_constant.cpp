// A global created from a non-defining declaration is `constant` exactly
// when clang's `isConstantStorage(Ctx, false, false)` holds: a reference,
// or a `const` object (or array of them) of non-class type
// ([basic.def]/2, [dcl.stc]/7, [basic.type.qualifier]/1). A `const` class
// object stays `global`: it may be dynamically initialized elsewhere.
struct S { int v; };
extern int &xr;
extern const int &cxr;
extern S &sr;
extern const int ci;
extern const int ca[2];
extern int *const cp;
extern const int *pc;
extern const S cs;
extern const S csa[2];
struct M { static const int m; static const S ms; };

int use() {
  return xr + cxr + sr.v + ci + ca[1] + *cp + *pc + cs.v + csa[1].v + M::m + M::ms.v;
}
