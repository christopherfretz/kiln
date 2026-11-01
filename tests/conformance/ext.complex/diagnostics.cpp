// GNU _Complex errors under clang 19 (WP-211): invalid `_Complex` bases
// (the `_Complex` is dropped), implicit complex-to-real conversions,
// operators complex types do not support, and constant evaluation
// failures (part overflow reports the element type; compound assignment
// on a complex object is not a constant expression in clang 19).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:42 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 37:15 note "in call to 'ovf()'"
//@ diag: 39:13 error "constexpr variable 'sc' must be initialized by a constant expression"
//@ diag: 38:36 note "subexpression not valid in a constant expression"
//@ diag: 39:18 note "in call to 'scomp()'"
//@ diag: 40:1 error "'_Complex bool' is invalid"
//@ diag: 41:1 error "'_Complex void' is invalid"
//@ diag: 41:15 error "variable has incomplete type 'void'"
//@ diag: 42:1 error "'_Complex char16_t' is invalid"
//@ diag: 43:1 error "'_Complex wchar_t' is invalid"
//@ diag: 44:1 error "'_Complex __int128' is invalid"
//@ diag: 45:1 error "'_Complex auto' is invalid"
//@ diag: 47:1 warning "plain '_Complex' requires a type specifier; assuming '_Complex double'"
//@ diag: 47:10 error "redefinition of 'D' as different kind of symbol"
//@ diag: 46:16 note "previous definition is here"
//@ diag: 47:11 error "expected ';' after top level declarator"
//@ diag: 48:12 error "implicit conversion from 'C' (aka '_Complex double') to 'double' is not permitted in C++"
//@ diag: 49:9 error "implicit conversion from 'CI' (aka '_Complex int') to 'int' is not permitted in C++"
//@ diag: 51:17 error "implicit conversion from 'C' (aka '_Complex double') to 'double' is not permitted in C++"
//@ diag: 51:23 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'C')"
//@ diag: 51:30 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'C')"
//@ diag: 51:37 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'int')"
//@ diag: 52:24 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'double')"
//@ diag: 53:24 error "invalid operands to binary expression ('CI' (aka '_Complex int') and 'int')"
using C = _Complex double;
using CI = _Complex int;
constexpr int ovf() { CI a = 2147483647; ++a; return __real__ a; }
static_assert(ovf());
constexpr C scomp() { C a = 2.0; a *= 3.0; return a; }
constexpr C sc = scomp();
_Complex bool b;
_Complex void v;
_Complex char16_t c16;
_Complex wchar_t wc;
_Complex __int128 i128;
_Complex auto au = 1.0;
typedef double D;
_Complex D td;
double d = C();
int i = CI();
void f(double);
void g(C c) { f(c); c % c; c < c; c << 1; }
bool h(C c) { return c > 1.0; }
int k(CI c) { return c & 1; }
