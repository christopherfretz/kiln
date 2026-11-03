// [basic.lookup.qual.general]/1: a declaration with no type whose qualified
// declarator-id names an incomplete class (`F::a();`, `F::b;`, `F::e = 1;`),
// or a declarator after an unknown type in one (`F::c d;`), is the single
// "incomplete type 'F' named in nested name specifier" with its note (clang
// prints the group twice: the name is parsed tentatively first), with no
// "expected unqualified-id" after it.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 19:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 18:8 note "forward declaration of 'F'"
//@ diag: 20:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 18:8 note "forward declaration of 'F'"
//@ diag: 21:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 18:8 note "forward declaration of 'F'"
//@ diag: 22:1 error "incomplete type 'F' named in nested name specifier" dup(clang)
//@ diag: 18:8 note "forward declaration of 'F'"
struct F;
F::a();
F::b;
F::c d;
F::e = 1;
