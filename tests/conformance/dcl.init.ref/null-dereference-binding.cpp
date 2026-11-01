// -Wnull-dereference on a direct reference binding (clang 19 SemaInit
// SK_BindReference, WP-191): binding a reference to `*p`, `p` a null
// pointer constant with parens and casts ignored, volatile or not; a
// binding through a converted temporary loads instead (the indirection
// warning).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 27:12 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 28:13 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 29:19 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 30:22 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 31:34 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 32:11 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 33:9 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 34:5 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 35:5 warning "binding dereferenced null pointer to reference has undefined behavior"
//@ diag: 36:9 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 36:9 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 37:20 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 37:20 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
struct S { int m; S() = default; S(const S &) = default; };
struct D : S {};
void g(int &);
void h(const int &);
void byval(int);
int f() {
  int &r = *(int *)0;
  int &r2 = *(int *)(0);
  const int &r3 = *(const int *)nullptr;
  volatile int &rv = *(volatile int *)0;
  int &&rr = static_cast<int &&>(*(int *)0);
  S &rs = *(D *)0;
  S s = *(S *)0;
  g(*(int *)0);
  h(*(int *)0);
  byval(*(int *)0);
  const long &rl = *(int *)0;
  int &ok = *(int *)1;
  int *p = 0;
  int &ok2 = *p;
  return r + r2 + r3 + rv + rr + rs.m + s.m + rl + ok + ok2;
}
