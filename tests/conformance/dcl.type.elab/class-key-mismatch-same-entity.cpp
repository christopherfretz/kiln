// [dcl.type.elab]/5, [basic.scope.scope]/4 (WP-2547): an
// elaborated-type-specifier whose class-key does not agree with the earlier
// declaration of the class is an error, and it still redeclares that class
// (with its class-key): one entity `w`, not a second one that made the later
// `w` ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:1 error "use of 'w' with tag type that does not match previous declaration"
//@ diag: 14:8 note "previous use is here"
//@ diag: 17:3 error "variable has incomplete type 'w'"
//@ diag: 14:8 note "forward declaration of 'w'"
//@ diag: 19:1 error "use of 'u' with tag type that does not match previous declaration"
//@ diag: 18:7 note "previous use is here"
struct w;
union w;
class w;
w z;
union u;
struct u *q;
u *r = q;
