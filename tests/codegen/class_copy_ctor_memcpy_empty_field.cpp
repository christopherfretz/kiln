// clang ConstructorMemcpyizer: an empty-for-layout member joins the run of
// memcpyable members but copies no bytes (`isEmptyFieldForLayout`); the
// memcpy spans the lowest- to the highest-offset non-empty member, and a run
// of only empty members emits nothing.
struct R { int* p; };
struct Pred {};
struct C { long a; C() = default; C(C const&) : a(0) {} };
struct FV { [[no_unique_address]] R base; [[no_unique_address]] Pred p; C c; };
struct T1 { int x; Pred p; C c; };
struct T2 { int x; [[no_unique_address]] Pred p; C c; };
struct T3 { [[no_unique_address]] Pred p; int x; C c; };
struct T4 { [[no_unique_address]] Pred p; [[no_unique_address]] Pred q; C c; };
void f(FV& v, T1& a, T2& b, T3& c, T4& d) {
  FV v2 = v;
  T1 a2 = a;
  T2 b2 = b;
  T3 c2 = c;
  T4 d2 = d;
}
