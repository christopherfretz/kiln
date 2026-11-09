// [dcl.fct]/17: types shall not be defined in return types. clang's
// GetDeclSpecTypeForDeclarator checks the result type once per function
// declarator chunk, so in `struct S {} f(), g();` each function declarator
// reports it (and a nested function type reports it again). A parameter of
// a function at namespace scope is a parameter type; a local function and a
// member function are result types too.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:8 error "'S1' cannot be defined in the result type of a function"
//@ diag: 32:8 error "'S2' cannot be defined in the result type of a function"
//@ diag: 33:16 error "'S4' cannot be defined in the result type of a function"
//@ diag: 34:8 error "'S5' cannot be defined in the result type of a function"
//@ diag: 34:8 error "'S5' cannot be defined in the result type of a function"
//@ diag: 35:8 error "'S6' cannot be defined in the result type of a function"
//@ diag: 35:8 error "'S6' cannot be defined in the result type of a function"
//@ diag: 36:19 error "'M::S7' cannot be defined in the result type of a function"
//@ diag: 37:19 error "'S8' cannot be defined in the result type of a function"
//@ diag: 37:27 warning "empty parentheses interpreted as a function declaration"
//@ diag: 37:27 note "remove parentheses to declare a variable"
//@ diag: 38:19 error "'S9' cannot be defined in the result type of a function"
//@ diag: 39:23 error "'S10' cannot be defined in a type specifier"
//@ diag: 39:23 error "'S10' cannot be defined in the result type of a function"
//@ diag: 40:23 error "'S11' cannot be defined in a condition"
//@ diag: 40:23 error "'S11' cannot be defined in the result type of a function"
//@ diag: 41:36 error "'S12' cannot be defined in a type alias template"
//@ diag: 41:36 error "'S12' cannot be defined in the result type of a function"
//@ diag: 42:8 error "'S13' cannot be defined in the result type of a function"
//@ diag: 43:1 error "'(unnamed struct at "
//@ diag: 44:15 error "'S14' cannot be defined in a parameter type"
struct S1 {} *f1();
struct S2 {} (*fp)();
typedef struct S4 {} F4();
struct S5 {} f5(int), f6(int);
struct S6 {} a6, f9(), b6, f10();
struct M { struct S7 {} m(); };
void k() { struct S8 {} lf(); }
using F9 = struct S9 {} ();
int a = sizeof(struct S10 {} (*)());
void c() { if (struct S11 {} (*p)() = 0) {} }
template<class T> using G = struct S12 {} ();
struct S13 {} (*fp13)(), v13;
struct {} g14();
void g(struct S14 {} (*p)());
int b = sizeof(S13) + sizeof(a6);
