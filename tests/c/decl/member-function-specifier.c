// C23 6.7.2.1p1: a member declaration takes a specifier-qualifier-list,
// so no function specifier (`inline`, `_Noreturn`) and no `constexpr`
// (clang 22 "type name does not allow function specifier to be
// specified"); the specifier is dropped and the member declared. Found
// running GCC's C++ testsuite (the rule kiln's C and C++ parsers share).
//@ std: c23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "type name does not allow function specifier to be specified"
//@ diag: 12:26 error "type name does not allow function specifier to be specified"
//@ diag: 12:43 error "type name does not allow constexpr specifier to be specified"
struct A { inline int x; _Noreturn int y; constexpr int w; int ok; };
int n = sizeof(struct A);
