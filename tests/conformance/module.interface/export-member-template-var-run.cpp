// [module.interface]/1, /6, [temp.static]: an exported class template's
// static data member defined out of class without `export` is usable from
// an importer (WP-1159). clang 19 rejects the importer's instantiation
// ("declaration of 'x' in the global module follows declaration in module
// member_defs"); clang 22 accepts it (docs/VALIDATION.md, known oracle
// defects).
//@ expect: run
//@ std: c++26
//@ requires: clang>=22
//@ module: member_defs=Inputs/member-defs.cppm
import member_defs;

int main() {
  if (X<2>::x != 2)
    return 1;
  X<3>::x = 4;
  return X<3>::x + X<2>{}.f() == 7 ? 0 : 2;
}
