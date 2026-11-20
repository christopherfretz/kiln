// [cpp.pragma]/1: #pragma clang section (clang ActOnPragmaClangSection): a pragma whose
// section's flags differ from an earlier declaration's or pragma's is an error
// and leaves the kind unchanged; a declaration conflicting with a section a
// pragma placed is reported with the pragma; malformed pragmas get clang's
// errors (the entries before the error take effect). WP-1744.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:23 error "this causes a section type conflict with 'a'"
//@ diag: 23:5 note "declared here"
//@ diag: 25:31 error "this causes a section type conflict with a prior #pragma section"
//@ diag: 25:23 note "#pragma entered here"
//@ diag: 27:5 error "'b' causes a section type conflict with a prior #pragma section"
//@ diag: 25:23 note "#pragma entered here"
//@ diag: 31:5 error "'d' causes a section type conflict with a prior #pragma section"
//@ diag: 29:23 note "#pragma entered here"
//@ diag: 32:48 error "this causes a section type conflict with a prior #pragma section"
//@ diag: 32:39 note "#pragma entered here"
//@ diag: 33:27 error "expected string literal in pragma clang section"
//@ diag: 34:27 error "expected string literal in pragma clang section"
//@ diag: 35:27 error "string literal with user-defined suffix cannot be used here"
//@ diag: 36:39 error "expected '=' following '#pragma clang section data'"
//@ diag: 37:23 error "expected one of [bss|data|rodata|text|relro] section kind in '#pragma clang section'"
int a __attribute__((section("A"))) = 1;
#pragma clang section bss="A"
#pragma clang section bss="C" data="C"
#pragma clang section rodata="R" relro="R"
int b __attribute__((section("C"))) = 1;
const int c __attribute__((section("R"))) = 1;
#pragma clang section text="T"
void f() __attribute__((section("T")));
int d __attribute__((section("T"))) = 1;
#pragma clang section bss="Z" bss="Z" data="Q" text = "Q"
#pragma clang section bss=u8"x"
#pragma clang section bss=L"x"
#pragma clang section bss="x"_s
#pragma clang section bss="x" "y" data
#pragma clang section ,
