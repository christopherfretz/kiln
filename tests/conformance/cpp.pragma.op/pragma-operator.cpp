// [cpp.pragma.op]: _Pragma("...") behaves as the #pragma it destringizes,
// including when it is produced by macro expansion.
//@ expect: compile-ok
//@ args: -Wunknown-pragmas
//@ strict-diags
//@ diag: 15:1 warning "unknown pragma ignored"
//@ diag: 12:22 note "expanded from macro 'DO_PRAGMA'"
//@ diag: 16:1 warning "#pragma once in main file"
//@ diag: 17:1 warning "missing '(' after '#pragma visibility' - ignoring"
//@ diag: 13:16 note "expanded from macro 'HIDDEN'"
//@ diag: 12:22 note "expanded from macro 'DO_PRAGMA'"
#define DO_PRAGMA(x) _Pragma(#x)
#define HIDDEN DO_PRAGMA(GCC visibility push hidden)
_Pragma("GCC visibility push(hidden)") int a; _Pragma("GCC visibility pop")
DO_PRAGMA(kiln frobnicate)
_Pragma("once")
HIDDEN
int b;
