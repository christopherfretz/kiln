// [conv.fpint]/1, [dcl.init.list]/7.2: a narrowing floating literal in a
// list-initialization is an error, and -Wliteral-conversion still reports
// its changed value.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:6 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 17:6 note "insert an explicit cast to silence this issue"
//@ diag: 17:6 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 18:17 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 18:17 note "insert an explicit cast to silence this issue"
//@ diag: 18:17 warning "implicit conversion from 'double' to 'int' changes value from 2.5 to 2"
//@ diag: 19:5 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 19:5 note "insert an explicit cast to silence this issue"
//@ diag: 19:5 warning "implicit conversion from 'double' to 'int' changes value from 3.5 to 3"
struct G { int a, b; };
struct H { G g; };
H h{{1.5, 2}};
int arr[] = {1, 2.5};
G g{3.5, 4};
