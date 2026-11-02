// [module.global.frag]/1, [basic.def.odr]/15, [class.mfct]/1: the
// non-inline definitions of an interface unit's global module fragment
// (a function, a variable, the key function of G and with it G's vtable)
// and the non-inline in-class member functions and vtables of its
// purview classes are emitted by that unit only; two importers using
// them link without duplicate definitions, and all three share the one
// variable. (clang 22 instead emits G's vtable in every importer: spec/
// modules.md §10 deviation 7.)
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/gd.cppm %{test_dir}/Inputs/gd-user.cpp
//@ args: -fmodule-file=gd=%{test_dir}/Inputs/gd.cppm
//@ stdout: 57 57 1 3 4 5 2
//@ skip(clang): clang needs a two-step --precompile build of the Inputs module that the harness cannot drive
import gd;
extern "C" int printf(const char *, ...);
int second_user();

int main() {
  G g;
  ML l;
  MK k;
  int a = second_user();
  int b = gmf_next() + 15;
  printf("%d %d %d %d %d %d %d\n", a, b, g.f(), l.f(), l.g(), k.f(), use_gi());
}
