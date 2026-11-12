// [dcl.pre]/10-11, [temp.inst]/4, [temp.arg.explicit]/1,
// [module.interface], spec/modules.md §11.11 (WP-1314): an imported
// function's block-scope static assertions are checked again by each
// instantiation, and a call through explicit template arguments names the
// same specialization; under `--bmi-image` the assertions are local
// records of the declaration image and the program behaves exactly as
// with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imglsa=Inputs/img-local-static-assert.cppm
import imglsa;
int main() {
  if (lsa::f(3) != 4 || lsa::f('a') != 98)
    return 1;
  if (lsa::g(2) != 4 || lsa::S<int>::h() != 7)
    return 2;
  if (lsa::callpick() != 11)
    return 3;
  return 0;
}
