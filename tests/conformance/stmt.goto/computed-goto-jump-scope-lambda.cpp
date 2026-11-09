// [stmt.goto] (GNU computed goto), [stmt.dcl]/3, [expr.prim.lambda]: the
// jump-scope check of a function walks the bodies of the lambdas it
// contains (clang `LambdaExpr::children` ends with the body), so an
// indirect `goto` may reach a label whose address a lambda takes, through
// the scopes of the lambda's declarations and of the variable it
// initializes; an indirect `goto` in a lambda is checked against the
// enclosing function's labels too, and a bad jump in a lambda is diagnosed
// by both functions (clang prints it twice). Default arguments and local
// classes are not walked, and a target in the lambda body's outermost
// scope is fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 54:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 57:3 note "possible target of indirect goto statement"
//@ diag: 56:11 note "jump bypasses variable initialization"
//@ diag: 55:8 note "jump bypasses variable initialization"
//@ diag: 62:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 66:3 note "possible target of indirect goto statement"
//@ diag: 65:11 note "jump bypasses variable initialization"
//@ diag: 64:7 note "jump bypasses variable initialization"
//@ diag: 71:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 76:5 note "possible target of indirect goto statement"
//@ diag: 75:13 note "jump bypasses variable initialization"
//@ diag: 74:11 note "jump bypasses variable initialization"
//@ diag: 82:3 error "indirect goto in function with no address-of-label expressions"
//@ diag: 89:17 error "indirect goto in function with no address-of-label expressions"
//@ diag: 89:17 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 92:1 note "possible target of indirect goto statement"
//@ diag: 91:9 note "jump bypasses variable initialization"
//@ diag: 90:7 note "jump bypasses variable initialization"
//@ diag: 101:31 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 98:1 note "possible target of indirect goto statement"
//@ diag: 101:10 note "jump exits scope of variable with non-trivial destructor"
//@ diag: 100:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 101:28 note "possible target of indirect goto statement"
//@ diag: 101:19 note "jump bypasses variable initialization"
//@ diag: 101:10 note "jump bypasses variable initialization"
//@ diag: 108:5 error "cannot jump from this goto statement to its label" dup(clang)
//@ diag: 109:9 note "jump bypasses variable initialization"
//@ diag: 116:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 120:3 note "possible target of indirect goto statement"
//@ diag: 119:11 note "jump bypasses variable initialization"
//@ diag: 118:7 note "jump bypasses variable initialization"
//@ diag: 116:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 124:15 note "in instantiation of function template specialization 'tmpl<int>' requested here"
//@ diag: 120:3 note "possible target of indirect goto statement"
//@ diag: 119:11 note "jump bypasses variable initialization"
//@ diag: 118:7 note "jump bypasses variable initialization"
struct D {
  D();
  ~D();
};
void var_init(void *p) {
  goto *p;
  auto l = [] {
    void *q = &&m;
  m:
    (void)q;
  };
}
void expr_stmt(void *p) {
  goto *p;
  [] {
    D d;
    void *q = &&m;
  m:
    (void)q;
  }();
}
void nested(void *p) {
  goto *p;
  [] {
    [] {
      int k = 1;
      void *q = &&m;
    m:
      (void)q, (void)k;
    }();
  }();
}
void not_walked(void *p) {
  goto *p;
  void g(int (*)() = [] { int k = 1; void *q = &&m; m: (void)q; return k; });
  struct L {
    void h() { int k = 1; void *q = &&m; m: (void)q, (void)k; }
  };
}
void lambda_jumps_out(void *p) {
  [](void *q) { goto *q; }(p);
  int k = 1;
  void *a = &&x;
x:
  (void)k, (void)a;
  goto *p;
}
void lambda_exits_dtor(void *p) {
  void *a = &&x;
x:
  (void)a;
  goto *p;
  [] { D d; void *q = &&m; m: goto *q; }();
}
void direct_goto(void *p) {
  void *a = &&x;
x:
  (void)a;
  [] {
    goto y;
    int k = 1;
  y:
    (void)k;
  }();
  goto *p;
}
template <class T> void tmpl(void *p) {
  goto *p;
  [] {
    T t = T();
    void *q = &&m;
  m:
    (void)q, (void)t;
  }();
}
template void tmpl<int>(void *);
void same_scope_ok(void *p) {
  goto *p;
  [] {
  m:
    void *q = &&m;
    (void)q;
  }();
}
