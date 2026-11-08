// [dcl.init.string]/1 under `-std=c++17 -fchar8_t` (clang `Char8` on before
// C++20): the same rules as C++20, the UTF-8-into-`signed char` error
// naming `-fchar8_t`. spec/sema.md "String literal initializers", WP-697
// (WP-823).
//@ expect: compile-fail
//@ args: -std=c++17 -fchar8_t
//@ strict-diags
//@ diag: 34:13 error "initialization of char array with UTF-8 string literal is not permitted by '-fchar8_t'"
//@ diag: 35:9 error "initializing 'char8_t' array with plain string literal"
//@ diag: 35:16 note "add 'u8' prefix to form a 'char8_t' string literal"
//@ diag: 36:9 error "initializing char array with wide string literal"
//@ diag: 37:9 error "initializing wide char array with non-wide string literal"
//@ diag: 38:9 error "initializing wide char array with non-wide string literal"
//@ diag: 39:9 error "initializing wide char array with incompatible wide string literal"
//@ diag: 40:10 error "initializing wide char array with incompatible wide string literal"
//@ diag: 41:10 error "initializing wide char array with incompatible wide string literal"
//@ diag: 42:6 error "initializing char array with wide string literal"
//@ diag: 43:6 error "initializing char array with wide string literal"
//@ diag: 44:12 error "initializing char array with wide string literal"
//@ diag: 45:15 error "initializing wide char array with non-wide string literal"
//@ diag: 48:17 error "cannot initialize an array element of type 'char8_t' with an lvalue of type 'const char[2]'"
//@ diag: 49:24 error "cannot initialize an array element of type 'const char16_t' with an lvalue of type 'const char8_t[2]'"
//@ diag: 51:8 error "cannot initialize an array element of type 'char8_t' with an lvalue of type 'const char[2]'"
//@ diag: 52:27 error "initializing 'char8_t' array with plain string literal"
//@ diag: 52:27 note "add 'u8' prefix to form a 'char8_t' string literal"
// Valid: matching kinds, and P2513's `char` / `unsigned char` from `u8`.
char ok1[] = u8"x";
unsigned char ok2[] = u8"x";
char8_t ok3[] = u8"x";
const wchar_t ok4[] = L"x";
char16_t ok5[] = {u"x"};
char32_t ok6[] = (U"x");
// [dcl.init.string]/1 mismatches, reported at the declarator.
signed char a3[] = u8"x";
char8_t b1[] = "x";
char8_t b2[] = u"x";
wchar_t c1[] = "x";
wchar_t c2[] = u8"x";
wchar_t c3[] = u"x";
char16_t c4[] = U"x";
char16_t c5[] = L"x";
char d1[] = L"x";
char d2[] = (u"x");
const char d3[] = U"x";
const wchar_t c7[] = "x";
// In braces a mismatched literal initializes the elements
// ([dcl.init.list]/3.3 applies only to an appropriately-typed literal).
char8_t b3[] = {"x"};
const char16_t c6[] = {u8"x"};
struct S { char8_t m[4]; };
S s = {"x"};
struct T { char8_t m[3] = "x"; };
