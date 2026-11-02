//@ expect: ast
//@ ast-implicit
//@ ast-filter: Shad
//@ xfail(kiln): injected-class-names are not declared yet (spec/sema-class.md §2.2); the shadow order matches
//@ requires: clang>=22
// [temp.inst]/3: a class-scope using-declaration of a class template is
// replayed in each instantiation. clang replays a non-dependent one from the
// pattern's shadow list, newest first, so the instantiation lists its shadows
// (UsingShadowDecls and ConstructorUsingShadowDecls) in reverse; a dependent
// one is looked up again, in declaration order.
struct ShadB {
  ShadB(int);
  ShadB(long);
  void f(int);
  void f(long);
};
template<class T> struct ShadD : ShadB {
  using ShadB::ShadB;
  using ShadB::f;
};
template<class T> struct ShadE : T {
  using T::f;
};
void ShadUse() {
  (void)sizeof(ShadD<int>);
  (void)sizeof(ShadE<ShadB>);
}
