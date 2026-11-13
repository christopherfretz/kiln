// [cpp.pragma]: '#pragma redefine_extname' renames the symbol of an
// extern "C" declaration declared before (unused, undefined) or after the
// pragma; a written asm label wins (clang ActOnPragmaRedefineExtname).
// WP-1399.
//@ expect: ir
extern "C" int declared_first(void);
#pragma redefine_extname declared_first renamed_first
#pragma redefine_extname declared_later renamed_later
extern "C" int declared_later(void);
#pragma redefine_extname var_later renamed_var
extern "C" int var_later;
#pragma redefine_extname labeled other
extern "C" int labeled() __asm__("lab");
int call() { return declared_first() + declared_later() + var_later + labeled(); }
