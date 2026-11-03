// [basic.indet]/1 before C++26: reading an uninitialized automatic
// variable is undefined behaviour, so kiln writes no erroneous value under
// `-std=c++23` (spec/codegen.md §7q): the IR equals clang's, a plain
// `alloca` with no store or `llvm.memset`.
//@ expect: ir
//@ std: c++23
//@ requires: clang>=22
//@ requires: llvm>=22
void use(void *);
struct P {
  char c;
  int i;
};
int f() {
  int x;
  int a[16];
  P p;
  use(a);
  use(&p);
  use(&x);
  return x;
}
