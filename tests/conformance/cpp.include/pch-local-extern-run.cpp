// [basic.link]/6, [cpp.include]: block-scope function and `extern`
// declarations in an included header name entities of their namespace,
// and the test file's namespace-scope definitions define them. With
// `--pch-diff`, the header's snapshot links them the same way
// (WP-1293).
//@ expect: run
#include "Inputs/local_extern.h"

namespace le {
int le_fn(int x) { return x * 10; }
int le_var = 3;
int le_member() { return 100; }
inline namespace in {
int le_inner() { return 1000; }
}
int le_decl() { return 7; }
}
int le_defined(int x) { return x + 20; }
long le_global = 5;

int main() {
  le::S s;
  int r = le::use_fn() + le::use_var() + s.m() + le::twice() + le::use_in() + le::use_decl() +
          use_global();
  return r == 40 + 3 + 103 + 13 + 1000 + 7 + 27 ? 0 : 1;
}
