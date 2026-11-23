// [temp.dep.candidate]/1, [over.match.oper]/3, [temp.res.general]/1,
// [module.interface], spec/modules.md §11.11: an imported template's
// dependent operator expressions (binary, unary, a rewritten `!=` from
// `==`, a fold-expression's operator) keep the non-member candidates the unqualified lookup found at
// the template's definition in the module (operators that are not
// exported and not in an associated namespace), also when it found none,
// and do not see the better ones the module declares after the
// definition. Under
// `--bmi-image` the sets come from the declaration image (WP-2021: the
// image dropped them, and the instantiation looked the operator up
// again, finding the later declarations).
//@ expect: run
//@ std: c++26
//@ module: imgoper=Inputs/img-oper-lookup.cppm
import imgoper;

int main() {
  on::S s{1};
  if (add(s, on::S{2}) != 3)
    return 1;
  if (sub(s) != 101)
    return 2;
  if (cpl(s) != 1001)
    return 3;
  if (ne(s))
    return 4;
  if (fsub(s) != 101)
    return 6;
  if (fmul(s) != 11)
    return 7;
  return mul(s) == 11 ? 0 : 5;
}
