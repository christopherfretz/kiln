// [dcl.fct]/17, [dcl.fct.default]: a default argument (and a noexcept
// operand or decltype in a parameter) lies in the function prototype scope,
// so a tag defined there "cannot be defined in a parameter type" (clang's
// ActOnTag streams the identifier, so an unnamed enum prints "(null)"). An
// unnamed class is not a prototype-scope declaration and is rejected as a
// type specifier. Both declarations of 'S7' are entered, so the second is a
// redefinition, and the enumerators of a rejected enumeration are not
// visible to the default argument, as in clang.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:29 error "'S5' cannot be defined in a parameter type"
//@ diag: 29:22 error "(null) cannot be defined in a parameter type"
//@ diag: 30:27 error "'E' cannot be defined in a parameter type"
//@ diag: 31:22 error "'(unnamed struct at "
//@ diag: 32:28 error "'S6' cannot be defined in a parameter type"
//@ diag: 33:29 error "'S7' cannot be defined in a parameter type"
//@ diag: 33:57 error "redefinition of 'S7'"
//@ diag: 33:29 note "previous definition is here"
//@ diag: 34:39 error "'S8' cannot be defined in a parameter type"
//@ diag: 35:47 error "'S9' cannot be defined in a parameter type"
//@ diag: 36:33 error "'S10' cannot be defined in a parameter type"
//@ diag: 37:32 error "'S11' cannot be defined in a parameter type"
//@ diag: 38:32 error "'S12' cannot be defined in a parameter type"
//@ diag: 39:17 error "'S20' cannot be defined in a parameter type"
//@ diag: 40:15 error "'E2' cannot be defined in a parameter type"
//@ diag: 40:27 error "use of undeclared identifier 'a2'"
void f1(int = sizeof(struct S5 {}));
void f2(int = sizeof(enum {a}));
void f3(int = sizeof(enum E {b}));
void f4(int = sizeof(struct {}));
void f5(int = (int)(struct S6 { operator int(); }){});
void f6(int = sizeof(struct S7 {}), int = sizeof(struct S7 {}));
struct M { void g(int = sizeof(struct S8 {})); };
template<class T> void f7(int = sizeof(struct S9 {}));
auto l = [](int = sizeof(struct S10 {})) {};
void f8(int x = alignof(struct S11 {}));
void f9(decltype(sizeof(struct S12 {})) x);
void f10(struct S20 { int x; } = {});
void f12(enum E2 { a2 } = a2);
