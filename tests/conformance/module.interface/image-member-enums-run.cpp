// [dcl.enum]/2-5, [temp.inst]/3, [temp.mem.enum], [module.interface],
// spec/modules.md §11.11 (WP-1310): the enumerations of an imported class
// template get their enumerators' values from each instantiation; under
// `--bmi-image` they come from the declaration image as patterns and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgme=Inputs/img-member-enums.cppm
import imgme;
int main() {
  if (me::A<int>::d != 10 || me::A<char>::c != 2 || me::A<int>::b != 2)
    return 1;
  if (me::A<int>::y != 12 || sizeof(me::A<int>::E) != sizeof(long))
    return 2;
  if (int(me::A<short>::S::q) != 3 || me::A<int>().get() != 15)
    return 3;
  if (me::is_int<int>::value != 1 || me::is_int<long>::value != 0)
    return 4;
  if (me::B<me::is_int<int>>::v != 2 || me::B<me::is_int<char>>::v != 1)
    return 5;
  return 0;
}
