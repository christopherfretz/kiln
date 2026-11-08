// [module.unit]/2,8, spec/modules.md §11.5: an interface unit and an
// implementation unit compiled separately. The implementation unit is
// compiled against iu's module file and defines what the interface only
// declares; the importer links with both objects.
//@ expect: run
//@ std: c++20
//@ module: iu=Inputs/iu.cppm
//@ unit: Inputs/iu-impl.cpp
//@ stdout: 33 7
import iu;
extern "C" int printf(const char *, ...);

int main() {
  Acc a;
  a.add(3);
  a.add(4);
  printf("%d %d\n", iu_value(), a.total);
}
