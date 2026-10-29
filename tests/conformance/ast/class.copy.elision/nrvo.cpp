//@ expect: ast
//@ ast-filter: nrvo
//@ skip(clang): clang's JSON dump omits nrvo_candidate; the golden was checked against the clang 19 text dump
// [class.copy.elision]/1.1: NRVO marking as clang 19 `Scope::updateNRVOCandidate`
// / `applyNRVO`. A return of a candidate keeps that variable's return slot in
// each scope out to the function scope and takes it from every other
// variable; any other return disables NRVO through its scope. On exit a
// scope marks its own candidate `nrvo` and passes its state to the parent.
// A `return` keeps `nrvo_candidate` only if its variable ends up `nrvo`.
// No oracle: clang's JSON dump omits `nrvo_candidate`; this golden was
// checked against the clang 19 text dump.
struct S { S(); S(const S&); ~S(); };
S nrvo1(bool b) { if (b) { S x; return x; } S y; return y; }
S nrvo2(bool b) { S x, y; if (b) return x; return y; }
S nrvo3(bool b) { S x; if (b) return S(); return x; }
S nrvo4(S p) { return p; }
S nrvo5(bool b) { S x; if (b) return S(); S y; return y; }
S nrvo6(bool b) { S x; { S y; if (b) return y; } return x; }
S nrvo7(bool b) { S x; if (b) return x; S y; return x; }
S nrvo8(int n) { S x; for (int i = 0; i < n; ++i) { if (i) return x; } return (x); }
S nrvo9(bool b) { S x; if (b) { S y; return y; } return x; }
const S nrvo10() { const S x; return x; }
S nrvo11() { static S x; return x; }
int nrvo12() { int x = 1; return x; }
S nrvo13(bool b) { while (b) { S x; return x; } return S(); }
S nrvo14(int n) { S x; switch (n) { case 1: { S y; return y; } default: break; } return x; }
S &nrvo15(S &r) { S &x = r; return x; }
