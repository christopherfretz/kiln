// [dcl.fct]/2: the decl-specifier of a parameter declarator with a
// trailing return type must be plain `auto`; a constrained `C auto` or a
// cv-qualified `auto` there is not a placeholder of an abbreviated function
// template ([dcl.fct]/22) and is reported as clang does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "function with trailing return type must specify return type 'auto', not 'const auto'"
//@ diag: 13:11 error "function with trailing return type must specify return type 'auto', not 'C auto'"
//@ diag: 14:3 error "function with trailing return type must specify return type 'auto', not 'C auto'"
//@ diag: 15:35 error "function with trailing return type must specify return type 'auto', not 'C auto'"
template <class T> concept C = true;
void f1(const auto (*cb)(int) -> int);
void f2(C auto (*cb)(int) -> int);
C auto f3() -> int;
void f4(auto (*cb)(int) -> int, C auto (*d)(int) -> C auto);
