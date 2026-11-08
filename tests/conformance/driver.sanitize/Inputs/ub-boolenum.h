// Loads for the bool and enum range checks (clang EmitScalarRangeCheck;
// WP-871).
enum E { A, B, C };
enum N { M = -3, P = 5 };
enum class SC { X, Y };
enum F : int { F0 };
enum Z {};
enum EB : bool { T1 };
struct S { bool b; E e; bool bf : 1; E ebf : 3; N n; };
bool g;
bool f1(bool *p) { return *p; }
E f2(E *p) { return *p; }
N f3(N &n) { return n; }
SC f4(SC *p) { return *p; }
F f5(F *p) { return *p; }
bool f6(S *s) { return s->b && s->bf; }
E f7(S *s) { return s->e; }
E f8(S s) { return s.ebf; }
int f9(S *s) { return s->n; }
bool f10() { return g; }
bool f11(bool b) { b |= g; return b; }
Z f12(Z *z) { return *z; }
EB f13(EB *e) { return *e; }
bool &r();
bool f14() { return r(); }
bool f15(S *s, bool S::*m) { return s->*m; }
bool f16(bool *p, int i) { return p[i]; }
