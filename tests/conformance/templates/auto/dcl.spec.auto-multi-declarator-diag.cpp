// [dcl.spec.auto.general]/8: inconsistent deduction across declarators.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:1 error "'auto' deduced as 'int' in declaration of 'a' and deduced as 'double' in declaration of 'b'"
//@ diag: 8:1 error "'auto' deduced as 'int' in declaration of 'c' and deduced as 'long' in declaration of 'd'"
auto a = 1, b = 2.0;
long l;
auto c = 1, *d = &l;
