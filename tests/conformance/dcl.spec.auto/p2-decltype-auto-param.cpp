// [dcl.spec.auto.general]/2-3, [dcl.fct]/22: `decltype(auto)` is never a
// parameter's placeholder, in a function declaration's clause, a nested
// prototype or a lambda (clang's "'decltype(auto)' not allowed in function
// prototype" / "in lambda parameter"). As the trailing return type of a
// parameter's function declarator it is reported at the decl-specifier
// `auto` (clang's location), and the parameter's type becomes `int`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:9 error "'decltype(auto)' not allowed in function prototype"
//@ diag: 16:20 error "'decltype(auto)' not allowed in function prototype"
//@ diag: 17:9 error "'decltype(auto)' not allowed in function prototype"
//@ diag: 18:14 error "'decltype(auto)' not allowed in lambda parameter"
//@ diag: 19:14 error "'decltype(auto)' not allowed in lambda parameter"
//@ diag: 20:25 error "'decltype(auto)' not allowed in function prototype"
void f1(decltype(auto) x);
void f2(void (*cb)(decltype(auto)));
void f3(auto (*cb)(int) -> decltype(auto));
auto l4 = [](decltype(auto) x) {};
auto l5 = [](auto (*cb)(int) -> decltype(auto)) {};
auto l6 = [](void (*cb)(decltype(auto))) {};
void f7(auto (*cb)(int) -> const auto);
void f8(auto (&cb)(int) -> auto&);
