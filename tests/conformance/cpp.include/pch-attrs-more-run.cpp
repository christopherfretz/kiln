// GNU and clang attributes of a header's declarations whose effects reach
// the including file's code (WP-1806): a `cleanup` variable in an inline
// body, a `pass_object_size` parameter, an `ms_struct` layout,
// `overloadable` C functions, an `alias` and an `ifunc` of functions
// defined here, and a `weak_import` variable no unit defines. With
// `--pch-diff`, the header's snapshot carries them.
//@ expect: run
//@ stdout: 5 4 10 12 101 202 9 8 0 12 2
#include "Inputs/pch_attrs_more.h"
extern "C" int printf(const char *, ...);
extern "C" int pam_real(int x) { return x * 3; }
extern "C" int pam_twice(int x) { return x * 2; }
extern "C" void *pam_resolve() { return (void *)pam_twice; }
extern "C" int pam_ov(int x) __attribute__((overloadable)) { return x + 100; }
extern "C" int pam_ov(long x) __attribute__((overloadable)) { return (int)x + 200; }

int main() {
  char buf[10];
  int c = pam::clean(4);
  int cl = pam::cleaned;
  printf("%d %d %d %d %d %d %d %d %d %d %d\n", c, cl, (int)pam::osz(buf), (int)sizeof(pam::MS),
         pam_ov(1), pam_ov(2L), pam::alias_of(3), pam::via_ifunc(4), &pam::weak_one != nullptr,
         pam::ann(3), pam::in_sec(1));
}
