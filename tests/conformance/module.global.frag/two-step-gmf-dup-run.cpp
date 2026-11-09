// [module.global.frag]/1, [basic.def.odr]/15, spec/modules.md §11.5:
// gmf-dup-run.cpp built separately. gd's object alone defines its
// non-inline global-module-fragment and purview definitions and the
// vtables keyed to them; this importer and gd-user.cpp, each compiled
// against gd's module file, link with it without duplicate definitions.
//@ expect: run
//@ std: c++20
//@ module: gd=Inputs/gd.cppm
//@ unit: Inputs/gd-user.cpp
//@ stdout: 57 57 1 3 4 5 2
//@ oracle: none
//@ skip(clang): clang emits G's vtable in every importer, which then fails to link (spec/modules.md §10 deviation 7)
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
