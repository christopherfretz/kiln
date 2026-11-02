// [dcl.attr.noreturn]/1, [dcl.fct.spec]/1: C11 `_Noreturn`, a clang
// extension in C++ ("'_Noreturn' is a C11 extension" under -pedantic). On
// a function it is the function's `[[noreturn]]` (`h` falls off its end
// after a call to `f1` without -Wreturn-type); elsewhere it is an error and
// dropped. Clang's tentative parse does not take it for a decl-specifier,
// so a parameter or block-scope declaration starting with it is an
// expression.
//@ expect: compile-fail
//@ args: -pedantic
//@ strict-diags
//@ diag: 32:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 33:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 33:11 warning "'_Noreturn' is a C11 extension"
//@ diag: 33:11 warning "duplicate '_Noreturn' declaration specifier"
//@ diag: 34:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 34:1 error "'_Noreturn' can only appear on functions"
//@ diag: 35:12 warning "'_Noreturn' is a C11 extension"
//@ diag: 35:32 warning "'_Noreturn' is a C11 extension"
//@ diag: 35:32 error "'_Noreturn' can only appear on functions"
//@ diag: 35:51 warning "'_Noreturn' is a C11 extension"
//@ diag: 35:51 error "'_Noreturn' can only appear on functions"
//@ diag: 36:9 warning "'_Noreturn' is a C11 extension"
//@ diag: 36:9 error "'_Noreturn' can only appear on functions"
//@ diag: 37:6 error "variable has incomplete type 'void'"
//@ diag: 37:8 error "expected expression"
//@ diag: 39:10 warning "'_Noreturn' is a C11 extension"
//@ diag: 39:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 41:12 error "expected expression"
//@ diag: 41:41 error "use of undeclared identifier 'loc'"
//@ diag: 42:1 warning "'_Noreturn' is a C11 extension"
//@ diag: 42:1 error "'_Noreturn' can only appear on functions"
_Noreturn void f1();
_Noreturn _Noreturn void f2();
_Noreturn int v1;
struct S { _Noreturn void m(); _Noreturn int fld; _Noreturn static int sv; };
typedef _Noreturn void T();
void g(_Noreturn int x);
int h() { f1(); }
template<_Noreturn int N> struct C { static const int v = N; };
int c = C<4>::v;
void k() { _Noreturn int loc = 0; (void)loc; }
_Noreturn auto l = [] {};
