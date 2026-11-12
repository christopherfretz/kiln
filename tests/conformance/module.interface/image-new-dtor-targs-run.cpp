// [expr.new], [expr.prim.id.dtor]/2, [temp.arg.explicit]/1,
// [module.interface], spec/modules.md §11.11 (WP-1320): an imported
// module's new-expressions (also using the implicitly declared
// allocation functions), pseudo-destructor calls and calls with explicit
// template arguments; under `--bmi-image` they come from the declaration
// image and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgndt=Inputs/img-new-dtor-targs.cppm
import imgndt;
struct Q {
  Q(int) noexcept;
};
struct R {
  R(int);
};
int main() {
  if (!ndt::nothrow_new<Q, int>::value || ndt::nothrow_new<R, int>::value)
    return 1;
  if (!ndt::nothrow_dtor<int>::value)
    return 2;
  int i = 3;
  ndt::destroy(&i);
  int *a = ndt::make(7);
  int *c = ndt::boxed(6);
  int r = *a + *c + ndt::sizes();
  delete a;
  delete c;
  return r == 24 ? 0 : 3;
}
