// [temp.names]/3: '<' after a name that is not a template is a
// relational operator, so a type cannot follow.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:2 error "expected unqualified-id"
struct N {};
N<int> n;
