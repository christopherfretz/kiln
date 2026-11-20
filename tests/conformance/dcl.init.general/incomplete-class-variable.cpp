// [dcl.init.general]/16, [basic.def]/5: a variable of class type must
// have a complete type whatever its initializer: "variable has
// incomplete type 'I'" with the forward declaration as a note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:14 error "variable has incomplete type 'I'"
//@ diag: 18:8 note "forward declaration of 'I'"
//@ diag: 19:22 error "variable has incomplete type 'I'"
//@ diag: 18:8 note "forward declaration of 'I'"
//@ diag: 19:32 error "variable has incomplete type 'I'"
//@ diag: 18:8 note "forward declaration of 'I'"
//@ diag: 19:42 error "variable has incomplete type 'I'"
//@ diag: 18:8 note "forward declaration of 'I'"
//@ diag: 19:47 error "variable has incomplete type 'I'"
//@ diag: 18:8 note "forward declaration of 'I'"
//@ diag: 19:57 warning "empty parentheses interpreted as a function declaration"
//@ diag: 19:57 note "replace parentheses with an initializer to declare a variable"
struct I;
void h() { I i(1); I j(1,2); I k{1,2}; I l; I m = 1; I n(); }
