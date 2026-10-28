// [dcl.enum]/2, [basic.scope]: enumerator names are declared once
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:16 error "redefinition of enumerator 'A'"
//@ diag: 6:10 note "previous definition is here"
enum E { A, B, A };
