// GNU `__extension__` before an asm-declaration ([dcl.asm]), at namespace
// scope: libstdc++ <iostream>:82 (`__extension__ __asm (".globl
// _ZSt21ios_base_library_initv");`, taken when the init_priority attribute
// is not usable). The first error of <iostream> after WP-237 made
// `__extension__ template` work (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
namespace std {
  __extension__ __asm (".globl _ZSt21ios_base_library_initv");
}
__extension__ asm(".text");
