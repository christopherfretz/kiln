// [dcl.fct.def.general] (GNU `naked`): a naked function returning a class
// indirectly keeps the `sret` parameter in its signature but, like clang
// (`EmitFunctionProlog` returns before marking the `sret` argument
// `noalias`), without `noalias`; there is no prologue storing it to a
// slot, and the body ends in `llvm.trap` + `unreachable`. Callers pass the
// `sret` slot as for any indirect return (spec/codegen.md "Inlining
// attributes on definitions").
struct Big {
  long a, b, c, d, e;
};
__attribute__((naked)) Big nk_big(int x) { asm("nop"); }
struct S {
  __attribute__((naked)) Big m(int a) { asm("nop"); }
};
Big use(S s) { return s.m(1); }
long use2() { return nk_big(2).c; }
