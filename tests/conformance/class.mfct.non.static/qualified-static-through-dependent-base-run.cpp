// [class.mfct.non.static]/2, [expr.prim.id.general]/3, [temp.dep.type]/6:
// in a member of a class template with a dependent base, `F::isT(i)`
// names an overload set of `F` with a static and a non-static member.
// `F` may turn out to be a base, so the reference waits for the
// instantiation; in `B<Other>` it is not one, and the call selects the
// static member without an object (clang's `CheckQualifiedMemberReference`:
// an implicit member reference that finds a static member is not an
// error). In `B<F>` `F` is the base and the non-static members are
// called on `*this`. LLVM's `Function::isTargetIntrinsic(IID)` and
// `ShuffleVectorInst::isExtractSubvectorMask(…)` in
// `BasicTTIImplBase<T>` (BasicTTIImpl.h). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 10 13 0
#include <cstdio>

struct F {
  int v = 7;
  int isT() const { return v; }
  static int isT(int i) { return i * 2; }
  template <class U> static int tm(U u) { return static_cast<int>(u) + 1; }
  int tm() const { return -v; }
};
template <class T> struct B : T {
  int g(int i) { return F::isT(i) + F::tm(i); }
  int h() { return F::isT() + F::tm(); }
};
struct Other {};

int main() {
  B<Other> a;
  B<F> b;
  std::printf("%d %d %d\n", a.g(3), b.g(4), b.h());
}
