// [temp.expl.spec]/14, [dcl.ambig.res]/1: `template<> X C<int>::x();` is a
// function declaration (not a value-initialized definition of the static
// data member `x`), so it redefines `x` as a different kind of symbol, as
// does an out-of-line function declaration naming any non-function member;
// `template<> X C<int>::y{};` is a definition, so a second one is a
// redefinition; and a declaration (no initializer) after the definition
// only redeclares it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:27 error "redefinition of 'x' as different kind of symbol"
//@ diag: 22:12 note "previous definition is here"
//@ diag: 31:27 error "redefinition of 'y'"
//@ diag: 28:27 note "previous definition is here"
//@ diag: 34:29 error "redefinition of 'count'"
//@ diag: 32:29 note "previous definition is here"
//@ diag: 36:8 error "redefinition of 'f' as different kind of symbol"
//@ diag: 35:16 note "previous definition is here"
struct X {
  X();
};
template <typename C> struct cache {
  static X x, y;
  static int count;
};
template <typename C> X cache<C>::x;
template <typename C> int cache<C>::count;
template <> X cache<int>::x();
template <> X cache<int>::y{};
template <> X cache<int>::y;
template <> X cache<int>::y;
template <> X cache<int>::y{};
template <> int cache<int>::count = 1;
template <> int cache<int>::count;
template <> int cache<int>::count = 2;
struct S { int f; };
int S::f();
