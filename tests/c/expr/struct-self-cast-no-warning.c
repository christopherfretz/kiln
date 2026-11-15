// Without -pedantic a structure cast to its own type is accepted silently
// (clang: an Extension diagnostic).
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
struct S { int a; };
struct S f(struct S s) { return (struct S)s; }
