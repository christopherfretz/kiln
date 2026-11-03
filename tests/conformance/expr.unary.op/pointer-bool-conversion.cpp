// [conv.bool], [expr.unary.op]/3: a function, an array or the address of an
// object named directly is never null, so its contextual or implicit
// conversion to bool warns (-Wpointer-bool-conversion; a function adds the
// '&' fix-it note, and the '()' note when callable with no arguments and
// returning bool); 'this' and the address of a reference warn under
// -Wundefined-bool-conversion. No warning for '&f', explicit casts,
// pointer variables and parameters, weak declarations, unevaluated
// operands, or an expression from a macro body (clang's
// DiagnoseAlwaysNonNullPointer; same text under clang 19).
//@ strict-diags
//@ diag: 60:30 warning "'this' pointer cannot be null in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 67:7 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 67:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 68:8 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 68:8 note "prefix with the address-of operator to silence this warning"
//@ diag: 69:12 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 69:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 70:10 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 70:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 71:10 warning "address of function 'h' will always evaluate to 'true'"
//@ diag: 71:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 72:7 warning "address of function 'pred' will always evaluate to 'true'"
//@ diag: 72:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 72:7 note "suffix with parentheses to turn this into a function call"
//@ diag: 73:7 warning "address of function 'pred_def' will always evaluate to 'true'"
//@ diag: 73:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 73:7 note "suffix with parentheses to turn this into a function call"
//@ diag: 74:7 warning "address of function 'S::sm' will always evaluate to 'true'"
//@ diag: 74:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 75:7 warning "address of function 'S::sbm' will always evaluate to 'true'"
//@ diag: 75:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 75:7 note "suffix with parentheses to turn this into a function call"
//@ diag: 76:7 warning "address of function 'N::nf' will always evaluate to 'true'"
//@ diag: 76:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 78:8 warning "address of 'x' will always evaluate to 'true'"
//@ diag: 79:10 warning "address of 's.mem' will always evaluate to 'true'"
//@ diag: 80:7 warning "address of array 'arr' will always evaluate to 'true'"
//@ diag: 81:9 warning "address of array 's.a' will always evaluate to 'true'"
//@ diag: 82:11 warning "address of array 'ps->a' will always evaluate to 'true'"
//@ diag: 83:8 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 84:17 warning "address of function 'f' will always evaluate to 'true'"
//@ diag: 64:18 note "expanded from macro 'WRAP'"
//@ diag: 84:17 note "prefix with the address-of operator to silence this warning"
//@ diag: 64:18 note "expanded from macro 'WRAP'"
//@ diag: 101:46 warning "address of function 'S::sm' will always evaluate to 'true'"
//@ diag: 102:13 note "in instantiation of function template specialization 'member_of<S>' requested here"
//@ diag: 101:46 note "prefix with the address-of operator to silence this warning"
void f();
bool pred();
bool pred_def(int = 0);
int h(int);
int x;
int arr[3];
extern void wf() __attribute__((weak));
struct S {
  int a[2];
  int mem;
  static void sm();
  static bool sbm();
  bool self() const { return this; }
};
namespace N { void nf(); }
#define IN_BODY if (pred) {}
#define WRAP(e) (e)

void conversions(S s, S *ps, int &r, int pa[3], void (*fp)()) {
  if (f) {}
  if (!f) {}
  bool b = f;
  (void)(f ? 1 : 2);
  while (h) {}
  if (pred) {}
  if (pred_def) {}
  if (S::sm) {}
  if (S::sbm) {}
  if (N::nf) {}
  if (::x, pred) {}
  if (&x) {}
  if (&s.mem) {}
  if (arr) {}
  if (s.a) {}
  if (ps->a) {}
  if (&r) {}
  bool c = WRAP(f);
  IN_BODY
  (void)b; (void)c;
}

void no_warning(int pa[3], void (*fp)()) {
  if (&f) {}
  if ((bool)f) {}
  if (static_cast<bool>(f)) {}
  bool c = &f;
  if (fp) {}
  if (pa) {}
  if (wf) {}
  (void)sizeof(f ? 1 : 2);
  (void)c;
}

template<class T> bool member_of(T) { return T::sm; }
bool inst = member_of(S{});
