// Errors around parenthesized type-ids and functional casts.
//@ expect: compile-fail
//@ diag: 10:21 error "invalid application of 'sizeof' to a function type"
//@ diag: 11:21 error "invalid application of 'sizeof' to a function type"
//@ diag: 12:11 error "excess elements in scalar initializer"
//@ diag: 13:14 error "expected '(' for function-style cast or type construction"
//@ diag: 14:11 error "unexpected type name 'T': expected expression"
typedef int T;
void g() {
  unsigned long a = sizeof(int());
  unsigned long b = sizeof(T(int));
  int c = int{1, 2};
  int d = int;
  int e = T + 1;
  (void)a; (void)b; (void)c;
}
