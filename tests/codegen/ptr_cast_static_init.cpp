// [basic.start.static]/2-3, WP-1295: a `reinterpret_cast` (or a C-style cast
// performing one) is not a core constant expression ([expr.const]/5.15), so
// none of these is constant-initialized, but clang's constant emitter folds
// past the cast (`ConstantEmitter::tryEmitPrivateForVarInit`), and so does
// kiln's re-fold: an object pointer cast to another object pointer type, an
// integer cast to a pointer, and an address cast to an integer as wide as a
// pointer, in a struct member too, are static data. The block-scope statics
// need no guard (spec/codegen.md "Static initialization").
using iptr = __INTPTR_TYPE__;
struct S { int a, b; } gs = {1, 2};
int g = 3;
char *p1 = reinterpret_cast<char *>(&gs.b) + 1;
void *p2 = reinterpret_cast<void *>(iptr(32));
iptr i1 = reinterpret_cast<iptr>(&g);
struct T { iptr x; int *p; } t = {(iptr)&gs, (int *)&gs + 1};
char *f() {
  static char *q = (char *)&gs;
  static iptr j = (iptr)&g;
  return q + j;
}
