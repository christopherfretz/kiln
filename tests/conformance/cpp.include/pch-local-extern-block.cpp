// [basic.link]/6, [cpp.include]: a block-scope declaration in the test
// file names the entity that an included header declares, at namespace
// scope or at block scope. With `--pch-diff`, the header's precompiled
// image links them the same way (WP-1293).
//@ expect: run
#include "Inputs/local_extern.h"

namespace le {
int user() {
  // The header's block-scope declarations.
  int le_fn(int);
  extern int le_var;
  int le_member(void);
  // The header's namespace-scope declaration.
  int le_decl(void);
  return le_fn(2) + le_var + le_member() + le_decl();
}
int le_fn(int x) { return x * 10; }
int le_var = 3;
int le_member() { return 100; }
int le_decl() { return 7; }
}
int main() {
  int le_defined(int);
  extern long le_global;
  return le::user() == 20 + 3 + 100 + 7 && le_global == 5 && le_defined(1) == 21 ? 0 : 1;
}
int le_defined(int x) { return x + 20; }
long le_global = 5;
