// [dcl.init.general]/6.3, [module.interface]: default-initialized
// `std::meta::info` constants of an imported module (the null
// reflection) are the null reflection in the importer, and select the
// same specialization as `info{}`.
//@ expect: run
//@ std: c++26
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: none
//@ module: ninfo=Inputs/null_info.cppm
//@ stdout: 1 3
import ninfo;
extern "C" int printf(const char *, ...);

static_assert(mnull == info{} && marr[1] == info() && mh.m == mnull);
static_assert(pick<mnull> == 1 && pick<^^int> == 2);

int main() {
  printf("%d %d\n", pick<mh.m>, mh.k);
  return 0;
}
