// C17 6.9.2p2 with -fcommon: a variable defined only by tentative
// definitions has common linkage, so the same tentative definition in
// two units links to one object (clang `isVarDeclStrongDefinition`).
// Without -fcommon (the default) it would be a duplicate symbol.
//@ expect: run
//@ args: -fcommon
//@ unit: Inputs/common-tentative-other.c
//@ stdout: 7 7 3
int printf(const char *, ...);
int shared;
int arr[3];
void set_shared(int v);
int main(void) {
  set_shared(7);
  arr[0] = 3;
  printf("%d %d %d\n", shared, shared, arr[0]);
  return 0;
}
