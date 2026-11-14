// [new.delete.single]/1, [basic.stc.dynamic.general]/2, [dcl.init.general]/16,
// [temp.res.general]: a header that redeclares the replaceable global
// allocation functions (the shape of <new>) and directly initializes a
// variable of a dependent type in a class template (the shape of
// <bits/postypes.h>). With `--pch-diff`, the header's snapshot
// links the redeclarations to the implicit functions and carries the
// dependent initializer (WP-1320).
//@ expect: run
#include "Inputs/global_new_paren.h"

void *operator new(gnp::size_t n, const gnp::nothrow_t &) noexcept {
  return ::operator new(n);
}

int main() {
  int *p = new int(3);
  int r = *p;
  delete p;
  int *q = new (gnp::nothrow) int(5);
  r += *q;
  delete q;
  int *v = new int[2]{4, 5};
  r += v[0] - 4;
  delete[] v;
  alignas(int) unsigned char buf[sizeof(int)];
  r += *::new (buf) int(6);
  gnp::pos<long> a(2);
  gnp::pos<long> b = a + 3;
  r += int(b.off) + int(gnp::pos<int>::make(7.5).off);
  return r == 3 + 5 + 6 + 5 + 7 ? 0 : 1;
}
