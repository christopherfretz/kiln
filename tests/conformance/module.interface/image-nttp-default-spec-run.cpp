// [temp.arg.general]/1, [temp.param]/12, [temp.expl.spec]/1,
// [module.interface], spec/modules.md §11.11 (WP-1320): explicit
// specializations of an imported class template that take its non-type
// default argument; under `--bmi-image` the default comes from the
// declaration image as an expression and the program behaves exactly as
// with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgnds=Inputs/img-nttp-default-spec.cppm
import imgnds;
int main() {
  nds::promote<int>::type d = 1.5;
  nds::promote<float>::type f = 2.5f;
  if (sizeof(nds::promote<double>::type) != sizeof(double))
    return 1;
  if (nds::twice<short>::value != 4 || nds::twice<char, 7>::value != 7)
    return 2;
  return int(d + f) - 4;
}
