// C17 6.7.2.3p6: an unnamed tag prints with its location. clang 19
// prints the keyword before the elaborated name and again inside it
// (`struct (unnamed struct at F:L:C)`), but once when it prints the
// canonical type (an `aka`); clang 22 always prints it once
// (`struct (unnamed at F:L:C)`). WP-1312, WP-1297 follow-up.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:7 error "initializing 'int' with an expression of incompatible type 'struct (unnamed struct at tests/c/decl/unnamed-tag-printing-19.c:13:1)'"
//@ diag: 18:5 error "assigning to 'int' from incompatible type 'union (unnamed union at tests/c/decl/unnamed-tag-printing-19.c:17:3)'"
//@ diag: 20:8 error "incompatible integer to pointer conversion initializing 'int *' with an expression of type 'enum (unnamed enum at tests/c/decl/unnamed-tag-printing-19.c:19:3)'"
//@ diag: 21:7 error "incompatible pointer to integer conversion initializing 'int' with an expression of type 'TP' (aka 'struct (unnamed at tests/c/decl/unnamed-tag-printing-19.c:14:9) *')"
struct { int a; } x;
typedef struct { int q; } *TP;
void f(void) {
  int i = x;
  union { int b; } u;
  i = u;
  enum { A } e;
  int *p = e;
  int j = (TP)0;
  (void)p; (void)j;
}
