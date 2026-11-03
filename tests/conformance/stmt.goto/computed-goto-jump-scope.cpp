// [stmt.goto] (GNU computed goto), [stmt.dcl]/3, [except.pre]/2: an
// indirect `goto` may jump to any label whose address the function takes,
// so each such target is checked like a `goto` from it (clang
// `JumpScopeChecker::VerifyIndirectJumps`): entering the scope of a
// variable with a non-trivial destructor, a variable length array or a
// try block, or leaving the scope of a variable with a non-trivial
// destructor (an indirect `goto` runs no cleanups), is an error with a
// note at the target and the scope. A function with no label address
// cannot contain `goto *`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-vla-cxx-extension
//@ diag: 35:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 33:3 note "possible target of indirect goto statement"
//@ diag: 32:7 note "jump bypasses variable with a non-trivial destructor"
//@ diag: 40:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 43:3 note "possible target of indirect goto statement"
//@ diag: 42:9 note "jump bypasses initialization of variable length array"
//@ diag: 49:3 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 51:3 note "possible target of indirect goto statement"
//@ diag: 50:3 note "jump bypasses initialization of try block"
//@ diag: 59:5 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 61:1 note "possible target of indirect goto statement"
//@ diag: 58:7 note "jump exits scope of variable with non-trivial destructor"
//@ diag: 63:22 error "indirect goto in function with no address-of-label expressions"
struct D {
  ~D();
};
void dtor(int i) {
  void *p = i ? &&a : &&b;
  {
    D d;
  a:;
  }
  goto *p;
b:;
}
void vla(int n) {
  void *p = &&in;
  goto *p;
  {
    int arr[n];
  in:
    (void)arr;
  }
}
void tryblk() {
  void *p = &&in;
  goto *p;
  try {
  in:;
  } catch (...) {
  }
}
void out_dtor(int i) {
  void *p = &&after;
  {
    D d;
    goto *p;
  }
after:;
}
void none(void *p) { goto *p; }
