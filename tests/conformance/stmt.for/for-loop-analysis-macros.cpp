// [stmt.for], [cpp.replace] (WP-419): -Wfor-loop-analysis does not skip
// macro expansions (clang 19 and 22 warn whether the 'for', its
// condition, its increment or the body's last statement comes from a
// macro, with an "expanded from macro" note); a 'for(;;)' without a
// condition, a condition variable, a 'while' loop and a variable
// written through a reference or whose address is taken in the body are
// not analyzed or do not warn; a dependent condition warns once per
// instantiation, with the instantiation note.
// See for-loop-analysis.cpp for the rules (spec/sema.md §7.11).
//@ args: -Wfor-loop-analysis
//@ strict-diags
//@ diag: 34:38 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 28:14 note "expanded from macro 'COND'"
//@ diag: 37:50 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 37:41 note "incremented here"
//@ diag: 31:13 note "expanded from macro 'INC'"
//@ diag: 38:48 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 32:16 note "expanded from macro 'LAST'"
//@ diag: 38:43 note "incremented here"
//@ diag: 39:13 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 33:41 note "expanded from macro 'WHOLE'"
//@ diag: 46:49 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 47:15 note "in instantiation of function template specialization 't1<int>' requested here"
//@ diag: 48:51 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 49:15 note "in instantiation of function template specialization 't2<int>' requested here"
//@ diag: 48:51 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 50:15 note "in instantiation of function template specialization 't2<long>' requested here"
#define COND i < n
#define FOR_EACH(v, n) for (int v = 0; v < n; ++v)
#define BODY { ++i; }
#define INC i++
#define LAST ++i;
#define WHOLE(n) for (int i = 0, j = 0; i < n; ++j) {}
void m2() { int i = 0, n = 3; for (; COND; ) {} }
void m3(int n) { FOR_EACH(i, n) { } }
void m4() { int i = 0, n = 3; for (; i < n; ) BODY }
void m5(int n) { for (int i = 0; i < n; INC) { ++i; } }
void m6(int n) { for (int i = 0; i < n; ++i) { LAST } }
void m7() { WHOLE(3) }
void s1() { int i = 0; for (;;) { if (i) break; } }
void s2() { int i = 0; for (;;) { (void)i; } }
void s3() { int i = 0, n = 2; for (; i < n; ) { int *p = &n; (void)p; } }
void s4() { int i = 0; for (; i < 3; ) { int &r = i; r = 1; } }
void s5() { int i = 0; for (; int x = i; ) { (void)x; } }
void s6() { int i = 0; while (i < 3) {} }
template <class T> void t1(T n) { for (T i = 0; i < n; ) {} }
template void t1<int>(int);
template <class T> void t2(T n) { for (int i = 0; i < n; ) {} }
template void t2<int>(int);
template void t2<long>(long);
