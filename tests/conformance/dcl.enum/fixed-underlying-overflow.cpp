// [dcl.enum]/5: an enumerator value must fit the fixed underlying type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:30 error "enumerator value evaluates to 300, which cannot be narrowed to type 'unsigned char'"
enum F : unsigned char { V = 300 };
