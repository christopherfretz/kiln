// [dcl.init.ref]/5 with CWG2352: similar pointer types that are
// reference-related but not reference-compatible (`const int *&` to an
// `int *`: the conversion is not a qualification conversion without
// `const` at every level above) cannot bind; nor does a non-const
// lvalue reference take a temporary of a similar type. clang's wording.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "binding reference of type 'const int *' to value of type 'int *' not permitted due to incompatible qualifiers"
//@ diag: 28:16 error "rvalue reference to type 'const int *' cannot bind to lvalue of type 'int *'"
//@ diag: 29:15 error "reference to type 'int *const' could not bind to an lvalue of type 'const int *'"
//@ diag: 30:15 error "binding reference of type 'const int *' to value of type 'int *const' drops 'const' qualifier"
//@ diag: 31:22 error "binding reference of type 'const int *const *' to value of type 'int **' not permitted due to incompatible qualifiers"
//@ diag: 32:15 error "non-const lvalue reference to type 'const int *' cannot bind to a temporary of type 'int *'"
//@ diag: 37:10 error "call to 'g2' is ambiguous"
//@ diag: 35:5 note "candidate function"
//@ diag: 36:5 note "candidate function"
//@ diag: 39:10 error "no matching function for call to 'h'"
//@ diag: 38:5 note "candidate function not viable: no known conversion from 'int *' to 'const int *&' for 1st argument"
//@ diag: 41:10 error "no matching function for call to 'k'"
//@ diag: 40:5 note "candidate function not viable: expects an rvalue for 1st argument"
int *p;
const int *cp;
int *const pc = nullptr;
int **pp;
void errors() {
  const int *&e1 = p;
  const int *&&e2 = p;
  int *const &e3 = cp;
  const int *&e4 = pc;
  const int *const *&e5 = pp;
  const int *&e6 = static_cast<int *&&>(p);
  volatile int *const volatile &e7 = p;
}
int g2(const int *const &);
int g2(const int *);
int a2 = g2(p);
int h(const int *&);
int a3 = h(p);
int k(const int *const &&);
int a4 = k(p);
