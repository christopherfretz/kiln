// [temp.explicit]/2, /10, [namespace.def.general]/2, [module.interface],
// spec/modules.md §11.11 "Explicit instantiations" (WP-1320): an
// explicit instantiation written in a later namespace block than the one
// that first named the specialization; under `--bmi-image` it comes from
// the declaration image and the program behaves exactly as with the
// replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgrei=Inputs/img-reopened-explinst.cppm
import imgrei;
int main() {
  rei::wide w{1};
  rei::basic<short> s{2};
  return w.width() == 81 && s.width() == 22 ? 0 : 1;
}
