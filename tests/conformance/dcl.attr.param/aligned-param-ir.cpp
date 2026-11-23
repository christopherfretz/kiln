// [dcl.attr.grammar]/5, [dcl.fct]: GNU `aligned(N)` on a parameter
// aligns the parameter's own storage in the callee (clang `EmitParmDecl`
// and `EmitFunctionProlog`, `getDeclAlign`): a scalar's spill slot and
// the memory a class passed in registers is stored into. spec/sema.md
// §4.5 "Parameter attributes", WP-2476.
//@ expect: ir
struct A { int x; };
int h(int w __attribute__((aligned(32))), int v) { return w + v; }
int k(A a __attribute__((aligned(64)))) { return a.x; }
