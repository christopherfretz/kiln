// C17 6.7.8p3: a typedef names an unnamed tag. clang's `printTag` prints
// such a tag by the typedef's name and without its keyword, so the type
// argument has no `aka` (WP-1312, WP-1296 follow-up); a pointer typedef
// desugars to the unnamed tag, which does not repeat its keyword.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:8 error "initializing 'int *' with an expression of incompatible type 'S'"
//@ diag: 19:8 error "incompatible integer to pointer conversion initializing 'int *' with an expression of type 'En'"
//@ diag: 21:8 error "initializing 'int *' with an expression of incompatible type 'U'"
//@ diag: 22:7 error "incompatible pointer to integer conversion initializing 'int' with an expression of type 'TP' (aka 'struct (unnamed at tests/c/decl/typedef-anonymous-tag-no-aka.c:14:9) *')"
typedef struct { int a; } S;
typedef enum { E0 } En;
typedef union { int u; } U;
typedef struct { int q; } *TP;
void f(void) {
  S s;
  int *p = s;
  En e = E0;
  int *q = e;
  U u;
  int *r = u;
  int j = (TP)0;
  (void)p; (void)q; (void)r; (void)j;
}
