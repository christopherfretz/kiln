// GNU `alias` and `ifunc` (spec/codegen.md "Aliases and ifuncs") on
// declarations attached to a named module ([module.unit]/7): the module
// unit defines the alias and the ifunc, and an importer only declares and
// calls them, whether it replays `imgam` or builds it from its
// declaration image (`--bmi-image`). kiln's importer checked them as its
// own definitions and rejected them as aliases of undefined symbols
// (WP-1817). Not clang-compared: clang 19 and 22 do not emit either into
// the object compiled from the module's .pcm, so their link fails.
//@ expect: run
//@ std: c++20
//@ oracle: none
//@ skip(clang): clang 19/22 do not emit a module unit's alias or ifunc from its .pcm
//@ module: imgam=Inputs/img-attrs-more.cppm
//@ stdout: 9 8
import imgam;
extern "C" int printf(const char *, ...);

int main() {
  printf("%d %d\n", iam::alias_of(3), iam::via_ifunc(4));
  return 0;
}
