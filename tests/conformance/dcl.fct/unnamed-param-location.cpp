// [dcl.fct]/3: an unnamed parameter is located where its name would be
// (clang's abstract-declarator identifier location: the first array bound,
// else the token after the declarator), as the "passing argument" note shows.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:5 error "conversion function from 'Ex' to 'int' invokes a deleted function"
//@ diag: 18:13 note "'operator int' has been explicitly marked deleted here"
//@ diag: 19:11 note "passing argument to parameter here"
//@ diag: 25:8 error "conversion function from 'Ex' to 'int *' invokes a deleted function"
//@ diag: 18:38 note "'operator int *' has been explicitly marked deleted here"
//@ diag: 20:26 note "passing argument to parameter here"
//@ diag: 26:5 error "conversion function from 'Ex' to 'int *' invokes a deleted function"
//@ diag: 18:38 note "'operator int *' has been explicitly marked deleted here"
//@ diag: 21:11 note "passing argument to parameter here"
//@ diag: 27:5 error "conversion function from 'Ex' to 'int' invokes a deleted function"
//@ diag: 18:13 note "'operator int' has been explicitly marked deleted here"
//@ diag: 22:17 note "passing argument to parameter here"
struct Ex { operator int() = delete; operator int*() = delete; };
void f(int);
void g(const int &, int *);
void h(int[3]);
void k(int const, int (*)(int));
void t(Ex e) {
  f(e);
  g(1, e);
  h(e);
  k(e, 0);
}
