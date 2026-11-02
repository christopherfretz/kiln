// GNU _Complex errors under clang 19 (WP-211): invalid `_Complex` bases
// (the `_Complex` is dropped), implicit complex-to-real conversions,
// operators complex types do not support, and constant evaluation
// failures (part overflow reports the element type).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:42 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 33:15 note "in call to 'ovf()'"
//@ diag: 36:1 error "'_Complex bool' is invalid"
//@ diag: 37:1 error "'_Complex void' is invalid"
//@ diag: 37:15 error "variable has incomplete type 'void'"
//@ diag: 38:1 error "'_Complex char16_t' is invalid"
//@ diag: 39:1 error "'_Complex wchar_t' is invalid"
//@ diag: 40:1 error "'_Complex __int128' is invalid"
//@ diag: 41:1 error "'_Complex auto' is invalid"
//@ diag: 43:1 warning "plain '_Complex' requires a type specifier; assuming '_Complex double'"
//@ diag: 43:10 error "redefinition of 'D' as different kind of symbol"
//@ diag: 42:16 note "previous definition is here"
//@ diag: 43:11 error "expected ';' after top level declarator"
//@ diag: 44:12 error "implicit conversion from 'C' (aka '_Complex double') to 'double' is not permitted in C++"
//@ diag: 45:9 error "implicit conversion from 'CI' (aka '_Complex int') to 'int' is not permitted in C++"
//@ diag: 47:17 error "implicit conversion from 'C' (aka '_Complex double') to 'double' is not permitted in C++"
//@ diag: 47:23 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'C')"
//@ diag: 47:30 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'C')"
//@ diag: 47:37 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'int')"
//@ diag: 48:24 error "invalid operands to binary expression ('C' (aka '_Complex double') and 'double')"
//@ diag: 49:24 error "invalid operands to binary expression ('CI' (aka '_Complex int') and 'int')"
using C = _Complex double;
using CI = _Complex int;
constexpr int ovf() { CI a = 2147483647; ++a; return __real__ a; }
static_assert(ovf());
// (Compound assignment on a complex object folds as in GCC since WP-380:
// compound-assign.cpp.)
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
