// [dcl.fct.default]/5, [over.match.list]/1: a default argument that is
// ill-formed is diagnosed where it is declared; the function remains
// callable, so an ill-formed argument at a later call is still
// diagnosed (clang marks only the parameter invalid).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "chosen constructor is explicit in copy-initialization"
//@ diag: 17:12 note "explicit constructor declared here"
//@ diag: 20:15 note "passing argument to parameter 'y' here"
//@ diag: 21:17 error "chosen constructor is explicit in copy-initialization"
//@ diag: 17:12 note "explicit constructor declared here"
//@ diag: 20:15 note "passing argument to parameter 'y' here"
//@ diag: 22:15 error "chosen constructor is explicit in copy-initialization"
//@ diag: 17:12 note "explicit constructor declared here"
//@ diag: 22:15 note "passing argument to parameter 'y' here"
struct E {
  explicit E(int);
  E(double);
};
void m(int, E y = {3});
void c() { m(1, {2}); }
void n(int, E y = {3});
void (*p)(int, E) = n;
