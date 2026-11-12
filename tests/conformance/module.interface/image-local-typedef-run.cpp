// [dcl.typedef]/1-2, [temp.inst]/4, [module.interface], spec/modules.md
// §11.11 (WP-1314): an imported function's block-scope typedef and alias
// declarations name their types in each instantiation; under
// `--bmi-image` they are local records of the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgltd=Inputs/img-local-typedef.cppm
import imgltd;
struct Q {
  using type = short;
};
int main() {
  if (ltd::f(3) != 7 || ltd::f('a') != 98)
    return 1;
  if (ltd::g(2) != 4 || ltd::S<Q>::h() != 2)
    return 2;
  return 0;
}
