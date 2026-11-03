// [over.over]/1, /3, /5: the implicit member overload set `f` (or `&f`,
// a lookup of the name) in a member function takes part in address
// resolution with its static member function templates: without a
// matching non-template, the specialization deduced from the target is
// selected (a non-static member, template or not, never matches a
// non-member target). With no match, `f` with a static member and `&f`
// report clang's "address of overloaded function ... does not match"
// with the candidate notes; `f` of only non-static members is a bound
// member function that must be called, and `&(f)` is not a pointer to
// member ([expr.unary.op]/4). The body is out of line so the candidates
// are in declaration order as clang lists them.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:24 error "address of overloaded function 'f' does not match required type 'void (float *)'"
//@ diag: 32:8 note "candidate function has type mismatch at 1st parameter (expected 'float *' but has 'int')"
//@ diag: 33:31 note "candidate template ignored: failed template argument deduction"
//@ diag: 45:25 error "address of overloaded function 'f' does not match required type 'void (float *)'"
//@ diag: 32:8 note "candidate function has type mismatch at 1st parameter (expected 'float *' but has 'int')"
//@ diag: 33:31 note "candidate template ignored: failed template argument deduction"
//@ diag: 46:25 error "address of overloaded function 'n' does not match required type 'void (float *)'"
//@ diag: 36:8 note "candidate function has type mismatch at 1st parameter (expected 'float *' but has 'int')"
//@ diag: 36:21 note "candidate function has type mismatch at 1st parameter (expected 'float *' but has 'long')"
//@ diag: 47:12 error "address of overloaded function 'f' cannot be static_cast to type 'void (*)(float *)'"
//@ diag: 32:8 note "candidate function"
//@ diag: 33:31 note "candidate function template"
//@ diag: 48:8 error "variable 'z' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 49:16 error "cannot create a non-constant pointer to member function"
//@ diag: 50:24 error "reference to non-static member function must be called"
//@ diag: 36:8 note "possible target for call"
//@ diag: 36:21 note "possible target for call"
struct S {
  void f(int);
  template <class T> static T f();
  template <class T> static void h(T);
  void h(char *);
  void n(int); void n(long);
  void k();
};
void S::k() {
  int (*a)() = f;
  int (*b)() = &f;
  void (*c)(double) = h;
  void (*d)(char *) = h;
  void (*g)(float *) = f;
  void (*i)(float *) = &f;
  void (*u)(float *) = &n;
  auto y = static_cast<void (*)(float *)>(f);
  auto z = &f;
  int (*t)() = &(f);
  void (*v)(float *) = n;
  (void)a; (void)b; (void)c; (void)d; (void)g; (void)i; (void)u; (void)t; (void)v;
}
