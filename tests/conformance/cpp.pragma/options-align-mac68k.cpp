// [cpp.pragma]/1: 'mac68k' alignment is Darwin's; on kiln's targets
// '#pragma options align=mac68k' is clang's error, and the pragma does
// nothing. WP-1744.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "mac68k alignment pragma is not supported on this target"
//@ diag: 12:9 warning "#pragma options align=reset failed: stack empty"
//@ diag: 13:9 error "mac68k alignment pragma is not supported on this target"
//@ diag: 14:9 warning "#pragma options align=reset failed: stack empty"
#pragma options align=mac68k
struct F { char c; int i; };
#pragma options align=reset
#pragma align=mac68k
#pragma align=reset
static_assert(sizeof(F) == 8);
