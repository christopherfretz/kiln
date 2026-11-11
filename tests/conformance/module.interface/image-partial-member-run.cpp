// [temp.spec.partial.member]/1, [temp.class.general]/3,
// [module.interface], spec/modules.md §11.11 "Templates as patterns"
// (WP-1307): members of a class template partial specialization
// defined outside it (a constructor, a member function, a static data
// member) are used by an importer; under `--bmi-image` they come from
// the declaration image and the program behaves exactly as with the
// replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgop=Inputs/img-partial-member.cppm
//@ stdout: 10 5 2
import imgop;
extern "C" int printf(const char *, ...);
int add(int a, int b) { return a + b; }
int main() {
  op::F<int(int, int)> f(add);
  printf("%d %d %d\n", op::use(), f.call(2, 3), op::F<int(int, int)>::count);
  return 0;
}
