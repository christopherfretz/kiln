// Classes with virtual bases: a delegating base-object constructor (the
// VTT passed through), implicit copy constructor and assignment, an
// abstract class (D1 traps, no vtable of its own), new/delete and
// virtual destruction through a virtual base (`_ZTv` thunks).
int n;
struct V { int v = 1; virtual ~V() { ++n; } };
struct L : virtual V { int l = 2; L() noexcept {} L(int x) noexcept : L() { l = x; } };
struct R : virtual V { int r = 3; };
struct D : L, R { int d = 4; D() noexcept : L(5) {} };
struct Abs : virtual V { virtual void p() = 0; Abs() noexcept {} };
struct Con : Abs { void p() override {} };
int f() noexcept {
  D a;
  D b = a;
  b = a;
  Con c;
  return a.d + b.l;
}
D *mk() { return new D; }
void kill(D *p) { delete p; }
V *mkl() { return new L(3); }
void killv(V *v) { delete v; }
