// [dcl.align]/1, /5: an alignment-specifier on a variable raises that
// variable's alignment (alignof of the id-expression, and its storage)
//@ expect: compile-ok
alignas(8) char g;
char g3 alignas(16);
static_assert(alignof(g) == 8);
static_assert(alignof(g3) == 16);
template <class... Ts> void f() {
  alignas(Ts...) char x;
  static_assert(alignof(x) == 8);
}
template void f<short, double>();
