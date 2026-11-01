// [except.handle], [dcl.attr.unused] (WP-217): -Wunused-exception-parameter
// (clang DiagnoseUnusedDecl, warn_unused_exception_param): a named
// exception variable never referenced is diagnosed when its handler's
// scope is left; not a placeholder `_`, a [[maybe_unused]] one, one whose
// class has a non-trivial destructor, nor a dependent one at
// instantiation. A reference to a class with a non-trivial destructor
// still counts.
//@ args: -Wunused-exception-parameter -Wunused-variable
//@ strict-diags
//@ diag: 29:16 warning "unused exception parameter 'x'"
//@ diag: 34:15 warning "unused exception parameter 'er'"
//@ diag: 35:20 warning "unused exception parameter 'p'"
//@ diag: 34:12 warning "exception of type 'E &' will be caught by earlier handler"
//@ diag: 33:12 note "for type 'E'"
//@ diag: 45:9 warning "unused variable 'z'"
//@ diag: 44:16 warning "unused exception parameter 'y'"
//@ diag: 52:16 warning "unused exception parameter 'e2'"
struct E {
  ~E();
};
struct P {
  int a;
};
void g();

void f() {
  try {
    g();
  } catch (int x) {
  }
  try {
    g();
  } catch (E e) {
  } catch (E &er) {
  } catch (const P p) {
  } catch ([[maybe_unused]] const long &l) {
  } catch (short _) {
  } catch (char c) {
    (void)c;
  } catch (...) {
  }
  try {
    g();
  } catch (int y) {
    int z;
  }
}

template <class T> void tf() {
  try {
    g();
  } catch (int e2) {
  } catch (T e) {
  }
}
template void tf<long>();
