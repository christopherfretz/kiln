// [dcl.fct]/5: a function template's parameter of a dependent array type
// is adjusted to a pointer. With `--pch-diff`, the header's precompiled
// snapshot carries the adjusted types (WP-1320).
//@ expect: run
#include "Inputs/dependent_array_params.h"

int main() {
  int a[2] = {3, 4};
  long b[] = {9, 1};
  short k[3] = {0, 0, 5};
  dap::holder<int> h{2};
  int r = dap::first2(a) + int(dap::head(b)) + h.scaled(k);
  return r == 26 ? 0 : 1;
}
