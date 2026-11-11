// C23 6.7.3.4p5 (tags): an enum tag that is not yet declared names no
// type in ISO C before C23 (an enum without fixed underlying type cannot be
// forward-declared); clang accepts it as an extension diagnosed under
// -pedantic.
//@ expect: compile-ok
//@ args: -pedantic
//@ strict-diags
//@ diag: 11:6 warning "ISO C forbids forward references to 'enum' types"
//@ diag: 13:6 warning "ISO C forbids forward references to 'enum' types"
//@ diag: 16:8 warning "ISO C forbids forward references to 'enum' types"
enum E;
enum E *p;
enum F *q;
enum E { A, B };
void g(void) {
  enum H *h = 0;
  (void)h;
}
int s = sizeof(enum E);
