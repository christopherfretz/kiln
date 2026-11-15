// [dcl.type.elab]/2 (WP-1448): an elaborated-type-specifier naming a
// typedef-name is diagnosed once per block: the invalid tag kiln declares
// to recover is found by later elaborated-type-specifiers in the block (as
// clang pushes it on the block scope), so a second `struct I` or a cast to
// `struct I *` gives no further error. Ordinary lookup still finds the
// typedef; a later block, or a block declaring the typedef itself, sees it
// again.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:10 error "typedef 'I' cannot be referenced with the 'struct' specifier"
//@ diag: 22:13 note "declared here"
//@ diag: 32:10 error "typedef 'X' cannot be referenced with the 'struct' specifier"
//@ diag: 23:22 note "declared here"
//@ diag: 37:12 error "typedef 'I' cannot be referenced with the 'struct' specifier"
//@ diag: 22:13 note "declared here"
//@ diag: 38:10 error "typedef 'I' cannot be referenced with the 'struct' specifier"
//@ diag: 22:13 note "declared here"
//@ diag: 40:10 error "typedef 'L' cannot be referenced with the 'struct' specifier"
//@ diag: 39:16 note "declared here"
//@ diag: 41:10 error "typedef 'L' cannot be referenced with the 'struct' specifier"
//@ diag: 39:16 note "declared here"
typedef int I;
typedef struct S0 {} X;
void f() {
  struct I *p;
  struct I *q;          // finds the recovery tag
  (void)(struct I *)0;
  struct I *r = p;
  I i = 0;              // still the typedef
}
void g() {
  struct X *a;
  struct X *c = (struct X *)0;
  { struct X *d = a; }
}
void h() {
  { struct I *a; }
  struct I *b;          // another block: diagnosed again
  typedef long L;
  struct L *l;
  struct L *m;          // the typedef is in this block: diagnosed again
}
