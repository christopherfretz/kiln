// __builtin_offsetof errors (clang BuildBuiltinOffsetOf and the parser's
// designator grammar).
//@ expect: compile-fail
//@ diag: 31:9 error "offsetof of incomplete type 'Inc'"
//@ diag: 26:8 note "forward declaration of 'Inc'"
//@ diag: 32:9 error "offsetof requires struct, union, or class type, 'int' invalid"
//@ diag: 33:9 error "no member named 'nope' in 'B'"
//@ diag: 34:34 error "offsetof requires array type, 'int' invalid"
//@ diag: 35:31 error "cannot compute offset of bit-field 'bf'"
//@ diag: 28:16 note "bit-field is declared here"
//@ diag: 36:33 error "array subscript is not an integer"
//@ diag: 37:35 error "offsetof requires struct, union, or class type, 'int' invalid"
//@ diag: 38:9 error "no member named 'zz' in 'N'"
//@ diag: 39:9 warning "on non-standard-layout type 'R'"
//@ diag: 41:29 error "expected ','"
//@ diag: 42:31 error "expected identifier"
//@ diag: 43:34 error "expected ']'"
//@ diag: 43:32 note "to match this '['"
//@ diag: 44:33 error "expected identifier"
//@ diag: 45:31 error "expected identifier"
//@ diag: 46:33 error "expected ')'"
//@ diag: 46:27 note "to match this '('"
//@ diag: 47:27 error "expected '(' after '__builtin_offsetof'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
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
