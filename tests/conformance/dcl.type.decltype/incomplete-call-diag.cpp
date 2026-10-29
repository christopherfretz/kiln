// [dcl.type.decltype]/2: the exemption covers only the outermost call (or
// the right operand of a comma); any other call needs a complete type
// ([expr.call]/7).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:16 error "member access into incomplete type 'In'"
//@ diag: 14:8 note "forward declaration of 'In'"
//@ diag: 16:10 error "calling 'make' with incomplete return type 'In'"
//@ diag: 15:4 note "'make' declared here"
//@ diag: 14:8 note "forward declaration of 'In'"
//@ diag: 17:10 error "calling 'make' with incomplete return type 'In'"
//@ diag: 15:4 note "'make' declared here"
//@ diag: 14:8 note "forward declaration of 'In'"
struct In;
In make();
decltype(make().x) b;
decltype(make(), 1) c;
decltype(1, make()) d();
