// __builtin_offsetof errors (clang BuildBuiltinOffsetOf and the parser's
// designator grammar).
//@ expect: compile-fail
//@ diag: 29:9 error "offsetof of incomplete type 'Inc'"
//@ diag: 24:8 note "forward declaration of 'Inc'"
//@ diag: 30:9 error "offsetof requires struct, union, or class type, 'int' invalid"
//@ diag: 31:9 error "no member named 'nope' in 'B'"
//@ diag: 32:34 error "offsetof requires array type, 'int' invalid"
//@ diag: 33:31 error "cannot compute offset of bit-field 'bf'"
//@ diag: 26:16 note "bit-field is declared here"
//@ diag: 34:33 error "array subscript is not an integer"
//@ diag: 35:35 error "offsetof requires struct, union, or class type, 'int' invalid"
//@ diag: 36:9 error "no member named 'zz' in 'N'"
//@ diag: 37:9 warning "offset of on non-standard-layout type 'R'"
//@ diag: 39:29 error "expected ','"
//@ diag: 40:31 error "expected identifier"
//@ diag: 41:34 error "expected ']'"
//@ diag: 41:32 note "to match this '['"
//@ diag: 42:33 error "expected identifier"
//@ diag: 43:31 error "expected identifier"
//@ diag: 44:33 error "expected ')'"
//@ diag: 44:27 note "to match this '('"
//@ diag: 45:27 error "expected '(' after '__builtin_offsetof'"
struct Inc;
struct N { int n; };
struct B { int bf : 3; int a[2]; int m; N s; };
struct R { int &r; int m; };
void f() {
  (void)__builtin_offsetof(Inc, x);
  (void)__builtin_offsetof(int, x);
  (void)__builtin_offsetof(B, nope);
  (void)__builtin_offsetof(B, m[1]);
  (void)__builtin_offsetof(B, bf);
  (void)__builtin_offsetof(B, a[1.0]);
  (void)__builtin_offsetof(B, s.n.q);
  (void)__builtin_offsetof(B, s.zz);
  (void)__builtin_offsetof(R, m);
  (void)sizeof(__builtin_offsetof(R, m));
  (void)__builtin_offsetof(B);
  (void)__builtin_offsetof(B, );
  (void)__builtin_offsetof(B, a[1);
  (void)__builtin_offsetof(B, a.);
  (void)__builtin_offsetof(B, 1);
  (void)__builtin_offsetof(B, m x);
  (void)__builtin_offsetof;
}
