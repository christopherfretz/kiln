// [cpp.pragma]: a malformed '#pragma redefine_extname' is ignored with a
// warning (clang PragmaRedefineExtnameHandler); one naming a declaration
// without C language linkage is not applied, with a warning at that
// declaration (before or after the pragma); an asm label written on the
// declaration wins (clang ActOnPragmaRedefineExtname). WP-1399.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:25 warning "expected identifier in '#pragma redefine_extname' - ignored"
//@ diag: 14:27 warning "expected identifier in '#pragma redefine_extname' - ignored"
//@ diag: 15:30 warning "extra tokens at end of '#pragma redefine_extname' - ignored"
//@ diag: 17:5 warning "#pragma redefine_extname is applicable to external C declarations only; not applied to function 'cxx_fn'"
//@ diag: 18:5 warning "#pragma redefine_extname is applicable to external C declarations only; not applied to variable 'cxx_var'"
#pragma redefine_extname
#pragma redefine_extname a
#pragma redefine_extname a b c
#pragma redefine_extname cxx_fn new_cxx
int cxx_fn();
int cxx_var;
#pragma redefine_extname cxx_var new_var
extern "C" int labeled() __asm__("lab");
#pragma redefine_extname labeled other
#pragma redefine_extname later_labeled other2
extern "C" int later_labeled() __asm__("lab2");
int use() { return labeled() + later_labeled(); }
