// C17 6.9.2p2 with GNU `common` (no -fcommon): a tentative definition
// whose variable carries the attribute has common linkage, so the same
// tentative definition in two units links to one object; on a `static`
// variable, a parameter or a local it is accepted and changes nothing
// (clang `handleCommonAttr`, `isVarDeclStrongDefinition`; gcc.c-torture
// compile/pr43288.c). On a function it is ignored with a warning.
//@ std: c17
//@ expect: run
//@ strict-diags
//@ unit: Inputs/common-attribute-other.c
//@ diag: 17:20 warning "'common' attribute only applies to variables"
//@ stdout: 7 3 5
int printf(const char *, ...);
int shared __attribute__((common));
static int a __attribute__((common));
void set_shared(int v);
int __attribute__((common)) f(int p __attribute__((common)));
int f(int p) {
  int l __attribute__((common)) = p;
  return l;
}
int main(void) {
  set_shared(7);
  a = 3;
  printf("%d %d %d\n", shared, a, f(5));
  return 0;
}
