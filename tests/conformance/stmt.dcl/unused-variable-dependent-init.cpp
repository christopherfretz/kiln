// [stmt.dcl], [dcl.attr.unused] (WP-217): -Wunused-variable for a local
// variable of non-dependent class type with a type-dependent initializer
// (clang ShouldDiagnoseUnusedDecl): diagnosed in the template definition
// unless the class declares a non-trivial constructor (the one chosen is
// not known yet) or the initializer is an unresolved construction with
// dependent arguments.
//@ args: -Wunused-variable
//@ strict-diags
//@ diag: 24:6 warning "unused variable 'q'"
//@ diag: 26:6 warning "unused variable 'q2'"
//@ diag: 27:6 warning "unused variable 'q3'"
//@ diag: 28:7 warning "unused variable 'i'"
struct Tr {
  int v;
};
struct NT {
  NT() = default;
  NT(int);
  NT(const NT &) = default;
  int v;
};

template <class T> void tf(T t) {
  Tr q = t;
  NT n = t;
  Tr q2(t);
  Tr q3{t};
  int i = t;
  Tr q4 = Tr(t);
}
