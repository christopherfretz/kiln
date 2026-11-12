// [dcl.fct]/5, [module.interface], spec/modules.md §11.11 (WP-1320): an
// imported module's function templates with dependent array parameters;
// under `--bmi-image` they come from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgdap=Inputs/img-dependent-array-params.cppm
import imgdap;
int main() {
  int a[2] = {3, 4};
  long b[] = {9, 1};
  short k[3] = {0, 0, 5};
  dap::holder<int> h{2};
  int r = dap::first2(a) + int(dap::head(b)) + h.scaled(k);
  return r == 26 ? 0 : 1;
}
