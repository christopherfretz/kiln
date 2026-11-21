// [dcl.inline]/6, [basic.def.odr]/14, spec/modules.md §7 "Imported
// `used` functions" (WP-1825): an importer that names an imported inline
// function with GNU `used` defines it `linkonce_odr` in a comdat and
// keeps it in `llvm.compiler.used`, with the `used` overloads of the
// name, before the declaration that names it, as clang 22 does when it
// loads the declaration (before, kiln only declared it, as the module
// unit's own; it linked against the module's copy). One it does not
// name is not emitted. The module, the two importers and their copies
// are one function. Under `--bmi-image` from the declaration image.
//@ expect: run
//@ std: c++20
//@ module: imgui=Inputs/img-used-inline.cppm
//@ unit: Inputs/img-used-inline-user.cpp
//@ stdout: 1 2 3 5 1 3
import imgui;
extern "C" int printf(const char *, ...);
fp user_one();
int user_pick();

int main() {
  fp p = &one;
  printf("%d %d %d %d %d %d\n", p(), pick(1), pick(1L), ns::deep(), p == user_one(), user_pick());
}
