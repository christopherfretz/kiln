// [expr.new], [expr.prim.id.dtor]/2, [temp.arg.explicit]/1,
// [basic.stc.dynamic.general]/2: new-expressions (also using the
// implicitly declared allocation functions), pseudo-destructor calls and
// calls with explicit template arguments in a header (the shapes of
// <type_traits>). With `--pch-diff`, the header's snapshot
// carries them (WP-1320).
//@ expect: run
#include "Inputs/new_dtor_targs.h"

struct Q {
  Q(int) noexcept;
};
struct R {
  R(int);
};
struct T {
  ~T() noexcept(false);
};

int main() {
  if (!ndt::nothrow_new<Q, int>::value || ndt::nothrow_new<R, int>::value)
    return 1;
  if (!ndt::nothrow_dtor<int>::value || ndt::nothrow_dtor<T>::value)
    return 2;
  int i = 3;
  ndt::destroy(&i);
  int *a = ndt::make(7);
  int *b = ndt::array<int>(3);
  int *c = ndt::boxed(6);
  alignas(int) unsigned char buf[sizeof(int)];
  int *d = ndt::placed(buf);
  ndt::counted *e = ndt::fresh();
  int r = *a + b[2] + *c + *d + e->a + ndt::sizes();
  delete a;
  delete[] b;
  delete c;
  delete e;
  return r == 33 ? 0 : 3;
}
