// [cpp.pragma]: '#pragma redefine_extname old new' (clang
// ActOnPragmaRedefineExtname) gives the extern "C" function 'old' the
// assembly name 'new', whether it is declared before or after the
// pragma. Found while testing PCH prefixes (WP-1394).
//@ expect: compile-ok
extern "C" int declared_first(void);
#pragma redefine_extname declared_first renamed_first
#pragma redefine_extname declared_later renamed_later
extern "C" int declared_later(void);
int call() { return declared_first() + declared_later(); }
