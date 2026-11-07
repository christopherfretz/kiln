// [dcl.ambig.res]/1, [temp.inst]/3: in `N c(N::US::Lazy);` the name
// `N::US::Lazy` is an enumerator of the member enumeration `US` of the
// base `G<N>`, so the statement declares a variable, not a function. The
// enumeration is not instantiated when the lookahead classifies the name,
// which is then looked up in its pattern. LLVM's
// `DomTreeUpdater DTU(DomTreeUpdater::UpdateStrategy::Lazy);`
// (CloningTest.cpp). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 1 0 2
#include <cstdio>
template <class D> struct G {
  enum class US : unsigned char { Eager = 0, Lazy = 1 };
  enum UU { X, Y, Z };
  explicit G(US s) : s(s) {}
  explicit G(UU u) : s(US(u)) {}
  US s;
};
struct DT : G<DT> {
  using G::G;
};
struct N : G<N> {
  explicit N(US s) : G(s) {}
  explicit N(UU u) : G(u) {}
};
int main() {
  DT a(DT::US::Lazy);
  N b(N::US::Lazy);
  N c(N::US::Eager);
  N d(N::UU::Z);
  std::printf("%d %d %d %d\n", (int)a.s, (int)b.s, (int)c.s, (int)d.s);
}
