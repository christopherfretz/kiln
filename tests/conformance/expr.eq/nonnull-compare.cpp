// [expr.eq]/4, [conv.bool]/1: GNU nonnull/returns_nonnull pointers compared with
// null or converted to bool (clang DiagnoseAlwaysNonNullPointer): a call to a
// returns_nonnull function, a parameter named by its own or its function's
// nonnull attribute on any declaration (not once assigned, incremented or its
// address taken earlier in the function), in an instantiated function template;
// a lambda converted to a function pointer, outside a template only (WP-421).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 62:7 warning "comparison of nonnull parameter 'p' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 58:32 note "declared 'nonnull' here"
//@ diag: 63:7 warning "nonnull parameter 'p' will evaluate to 'true' on first encounter [-Wpointer-bool-conversion]"
//@ diag: 58:32 note "declared 'nonnull' here"
//@ diag: 64:12 warning "comparison of nonnull parameter 'p' not equal to a null pointer is 'true' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 58:32 note "declared 'nonnull' here"
//@ diag: 68:7 warning "comparison of nonnull parameter 'b' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 59:40 note "declared 'nonnull' here"
//@ diag: 69:8 warning "nonnull parameter 'b' will evaluate to 'true' on first encounter [-Wpointer-bool-conversion]"
//@ diag: 59:40 note "declared 'nonnull' here"
//@ diag: 72:7 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 60:24 note "declared 'nonnull' here"
//@ diag: 77:7 warning "comparison of nonnull function call 'rn()' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 56:26 note "declared 'returns_nonnull' here"
//@ diag: 78:7 warning "comparison of nonnull function call 'rn2(1)' not equal to a null pointer is 'true' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 57:16 note "declared 'returns_nonnull' here"
//@ diag: 79:7 warning "nonnull function call 'rn()' will evaluate to 'true' on first encounter [-Wpointer-bool-conversion]"
//@ diag: 56:26 note "declared 'returns_nonnull' here"
//@ diag: 80:13 warning "nonnull function call 'rn2(2)' will evaluate to 'true' on first encounter [-Wpointer-bool-conversion]"
//@ diag: 57:16 note "declared 'returns_nonnull' here"
//@ diag: 81:7 warning "comparison of nonnull function call '(rn())' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 56:26 note "declared 'returns_nonnull' here"
//@ diag: 83:7 warning "address of lambda function pointer conversion operator will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 84:18 warning "address of lambda function pointer conversion operator will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 85:12 warning "address of lambda function pointer conversion operator will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 95:28 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 94:32 note "declared 'nonnull' here"
//@ diag: 97:31 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 96:32 note "declared 'nonnull' here"
//@ diag: 99:36 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 98:32 note "declared 'nonnull' here"
//@ diag: 101:38 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 100:32 note "declared 'nonnull' here"
//@ diag: 101:56 warning "nonnull parameter 'q' will evaluate to 'true' on first encounter [-Wpointer-bool-conversion]"
//@ diag: 100:32 note "declared 'nonnull' here"
//@ diag: 103:25 warning "comparison of nonnull parameter 'p' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 102:42 note "declared 'nonnull' here"
//@ diag: 104:25 warning "comparison of nonnull parameter 'p' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 102:83 note "declared 'nonnull' here"
//@ diag: 107:23 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 106:17 note "declared 'nonnull' here"
//@ diag: 105:27 note "expanded from macro 'NN'"
//@ diag: 109:23 warning "comparison of nonnull parameter 'q' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 108:32 note "declared 'nonnull' here"
//@ diag: 89:40 warning "comparison of nonnull parameter 'p' equal to a null pointer is 'false' on first encounter [-Wtautological-pointer-compare]"
//@ diag: 91:22 note "in instantiation of function template specialization 'tp<int>' requested here"
//@ diag: 88:49 note "declared 'nonnull' here"
int *rn() __attribute__((returns_nonnull));
__attribute__((returns_nonnull)) int *rn2(int);
void p1(int *p) __attribute__((nonnull));
void p2(int *a, int *b) __attribute__((nonnull(2)));
void p3(__attribute__((nonnull)) int *q);
void p1(int *p) {
  if (p == nullptr) {}
  if (p) {}
  bool b = p != 0;
}
void p2(int *a, int *b) {
  if (a == nullptr) {}
  if (b == nullptr) {}
  if (!b) {}
}
void p3(int *q) {
  if (q == 0) {}
  q = nullptr;
  if (q == 0) {}
}
void calls() {
  if (rn() == nullptr) {}
  if (rn2(1) != 0) {}
  if (rn()) {}
  bool c = !rn2(2);
  if ((rn()) == nullptr) {}
  auto l = [] { return 1; };
  if (l == nullptr) {}
  if (nullptr != l) {}
  bool d = [](int) {} == 0;
  if (+l == nullptr) {}
}
template <class T> void tp(T *p) __attribute__((nonnull));
template <class T> void tp(T *p) { if (p == nullptr) {} }
template <class T> void tl() { auto l = [] {}; if (l) {} if (l == nullptr) {} }
void inst() { int i; tp<int>(&i); tl<int>(); }
void f1(int *q) __attribute__((nonnull));
void f1(int *q) { int **pp = &q; if (q == 0) {} }
void f2(int *q) __attribute__((nonnull));
void f2(int *q) { q++; if (q == 0) {} }
void f3(int *q) __attribute__((nonnull));
void f3(int *q) { q += 1; if (q == 0) {} }
void f4(int *q) __attribute__((nonnull));
void f4(int *q) { int *&r = q; if (q == 0) {} }
void f5(int *q) __attribute__((nonnull));
void f5(int *q) { auto g = [&] { if (q == 0) {} }; if (q) {} }
struct S { void m(int *p) __attribute__((nonnull)); void n(int *p) __attribute__((nonnull(2))); };
void S::m(int *p) { if (p == 0) {} }
void S::n(int *p) { if (p == 0) {} }
#define NN __attribute__((nonnull))
void f6(int *q) NN;
void f6(int *q) { if (q == 0) {} }
void f7(int *q) __attribute__((nonnull(1)));
void f7(int *q) { if (q == 0) {} }
