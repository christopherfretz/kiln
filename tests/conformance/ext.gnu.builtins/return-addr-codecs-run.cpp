// GNU builtins (an extension): `__builtin_extract_return_addr(p)` decodes
// a `__builtin_return_address` value and `__builtin_frob_return_addr`
// encodes one (both the identity on AArch64 and x86-64), and
// `__builtin___clear_cache(b, e)` flushes the instruction cache for
// [b, e). gcc.c-torture: compile/20040202-1.c, compile/pr100316.c.
//@ expect: run
//@ stdout: 1 1 1
#include <cstdio>
static char buf[64];
__attribute__((noinline)) void *caller() {
  return __builtin_extract_return_addr(__builtin_return_address(0));
}
int main() {
  void *r = caller();
  void *p = &buf[3];
  __builtin___clear_cache(buf, buf + sizeof buf);
  std::printf("%d %d %d\n", r != nullptr, __builtin_frob_return_addr(p) == p,
              __builtin_extract_return_addr(__builtin_frob_return_addr(p)) == p);
}
