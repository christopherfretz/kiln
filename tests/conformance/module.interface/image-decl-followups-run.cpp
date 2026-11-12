// [temp.names]/3, [temp.res.general], [temp.explicit], [module.interface],
// spec/modules.md §11.11 (WP-1353): an imported module's class templates
// named through a qualifier in dependent types and a deprecated explicit
// instantiation declaration; under `--bmi-image` they come from the
// declaration image and the program behaves exactly as with the replayed
// source.
//@ expect: run
//@ std: c++26
//@ module: imgdf=Inputs/img-decl-followups.cppm
import imgdf;

int main() {
  static_assert(df::ptr_or_ptr<int>::value && df::ptrish<int *> && !df::ptrish<int>);
  df::wrap<int> w{{3}};
  df::old<int> o{4};
  return w.b.v + o.get() == 7 ? 0 : 1;
}
