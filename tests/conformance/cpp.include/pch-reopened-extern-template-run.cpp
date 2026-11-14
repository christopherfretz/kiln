// [temp.explicit]/2, /10, [namespace.def.general]/2: explicit
// instantiations written in a later block of a namespace than the one
// that first named the specialization (the shape of <bits/basic_ios.tcc>).
// With `--pch-diff`, the header's snapshot carries them in
// source order after the walk (WP-1320).
//@ expect: run
//@ stdout: 12 81
#include "Inputs/reopened_extern_template.h"
extern "C" int printf(const char *, ...);

template struct ret::basic<char>;

int main() {
  ret::narrow n{1};
  ret::wide w{1};
  printf("%d %d\n", ret::twice(n) - 10, w.width());
  return 0;
}
