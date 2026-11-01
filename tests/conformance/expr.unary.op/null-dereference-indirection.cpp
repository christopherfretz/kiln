// -Wnull-dereference (clang 19 CheckForNullPointerDereference in
// DefaultLvalueConversion and CheckAssignmentOperands, WP-191): a load from
// or builtin assignment to `*p`, `p` a null pointer constant with parens and
// casts ignored, unless volatile. Not for member access, subscripts, `&*`,
// discarded or unevaluated operands, `++`, or unreachable code; a template
// definition warns, a dependent dereference per instantiation.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 34:11 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 34:11 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 35:11 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 35:11 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 36:12 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 36:12 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 37:16 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 37:16 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 39:12 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 39:12 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 40:7 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 40:7 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 41:15 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 41:15 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 42:3 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 42:3 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 43:3 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 43:3 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 57:42 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 57:42 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 56:39 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 58:10 note "in instantiation of function template specialization 'dep<int>' requested here"
//@ diag: 56:39 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
struct S { int m; };
int f(int c) {
  int a = *(int *)0;
  int b = *(int *)nullptr;
  int x = (*(int *)0);
  int y = (int)*(int *)0;
  int w = *(int *)(1 - 1);
  bool t = *(bool *)0 ? 1 : 0;
  if (*(int *)0) {}
  int q = 1 + *(int *)0;
  *(int *)0 = 1;
  *(int *)0 += 1;
  ++*(int *)0;
  volatile int v = *(volatile int *)0;
  int d = ((S *)0)->m;
  int e = (*(S *)0).m;
  int arr = ((int *)0)[0];
  (void)*(int *)0;
  int g = sizeof(*(int *)0);
  decltype(*(int *)0) h = a;
  int *p = &*(int *)0;
  if (false) return *(int *)0;
  return a + b + x + y + w + t + q + v + d + e + arr + g + h + *p;
}
template <class T> int dep() { return *(T *)0; }
template <class T> int nondep() { return *(int *)0; }
int i1 = dep<int>();
int i2 = 0;
constexpr int k() { return 1; }
