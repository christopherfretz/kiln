// [dcl.ref]/6, [conv.bool]: the address of a call returning a reference
// (a glvalue call: a function, member function, overloaded operator or
// function pointer) cannot be null in well-defined code, so its
// conversion to bool warns (-Wundefined-bool-conversion) at the call's
// location (the member name, the operator), with a note at a direct
// callee ("'f' returns a reference"; none through a pointer). The
// address of a function returning a prvalue is not diagnosed. Same text
// under clang 19 (clang's CheckForReference).
//@ strict-diags
//@ diag: 37:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 27:6 note "'f' returns a reference"
//@ diag: 38:15 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 30:8 note "'m' returns a reference"
//@ diag: 39:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 31:8 note "'operator*' returns a reference"
//@ diag: 40:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 32:15 note "'sm' returns a reference"
//@ diag: 41:14 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 27:6 note "'f' returns a reference"
//@ diag: 42:14 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 27:6 note "'f' returns a reference"
//@ diag: 43:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 44:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 27:6 note "'f' returns a reference"
//@ diag: 50:27 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true"
//@ diag: 30:8 note "'m' returns a reference"
int &f();
int g();
struct S {
  int &m();
  int &operator*();
  static int &sm();
};
int &(*fp)();

void use(S s, bool k) {
  bool a = &f();
  bool b = &s.m();
  bool c = &*s;
  bool d = &S::sm();
  bool e = !&f();
  bool h = &(f());
  bool i = &fp();
  bool j = &f() && k;
  bool m = &g;
  (void)a, (void)b, (void)c, (void)d, (void)e, (void)h, (void)i, (void)j,
      (void)m;
}

bool ret(S s) { return &s.m(); }
