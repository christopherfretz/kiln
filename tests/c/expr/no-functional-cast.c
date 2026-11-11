// C23 6.5.2.5, 6.5.4: C has no functional cast or T{...} list
// conversion, so a type-specifier or typedef name never begins an
// expression: `int(1)`, `typeof(int){1}` and `T(4)` are errors (clang
// "expected expression"); a compound literal names its type in
// parentheses, `(typeof(int)){1}` (WP-1296).
//@ std: c23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:9 error "expected expression"
//@ diag: 14:9 error "expected expression"
//@ diag: 15:9 error "unexpected type name 'T': expected expression"
typedef int T;
int a = int(1);
int b = typeof(int){1};
int c = T(4);
int ok(void) {
  int x = (typeof(int)){2};
  int *p = &(typeof(int)){3};
  return x + *p + (int)sizeof((typeof(int)){4}) + (T){5};
}
