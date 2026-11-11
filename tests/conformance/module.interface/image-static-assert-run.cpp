// [dcl.pre]/10, [temp.inst]/3, [module.interface], spec/modules.md
// §11.11 "Static assertions in class templates" (WP-1307): the static
// assertions of a module's class templates and their member classes
// are checked again by an importer's instantiations; under
// `--bmi-image` they come from the declaration image and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgsat=Inputs/img-static-assert.cppm
//@ stdout: 7 5 2
import imgsat;
extern "C" int printf(const char *, ...);
int main() {
  sat::Box<int> b{5};
  sat::Outer<short>::In i{2};
  printf("%d %d %d\n", sat::use(), b.get(), (int)i.w);
  return 0;
}
