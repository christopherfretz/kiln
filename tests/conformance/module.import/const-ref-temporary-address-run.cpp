// [class.temporary]/6, [basic.link]/11: the temporary a module's exported
// reference extends is one object, so an importer sees the module unit's
// address. kiln gives the temporary of an importable variable an
// external symbol in the module unit, which an importer declares
// (WP-1653). clang 22 makes it internal in the module unit and
// `available_externally` in the importer: an undefined reference at the
// link.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang 22 fails to link the importer of d4
//@ module: const_refs=Inputs/const-refs.cppm
//@ stdout: 1 1 sum 7
import const_refs;
extern "C" int printf(const char *, ...);

int main() {
  const void *a[] = {&d4, &d5};
  for (int i = 0; i < 2; ++i)
    printf("%d ", a[i] == module_address(i));
  printf("sum %d\n", d4 + d5);
}
