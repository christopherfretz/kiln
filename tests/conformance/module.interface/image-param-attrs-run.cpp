// [dcl.attr.grammar]/5, [module.interface], spec/modules.md §11.11
// (WP-1314): an imported function's parameter attributes
// (`[[clang::lifetimebound]]`, `[[maybe_unused]]`) are carried by the
// declaration image under `--bmi-image`.
//@ expect: run
//@ std: c++20
//@ module: imgpa=Inputs/img-param-attrs.cppm
import imgpa;
int main() {
  int x = 3;
  const int &r = pa::pick(x, 0);
  pa::S s{0};
  return r + pa::fwd(x) + s.get(x) - 9;
}
