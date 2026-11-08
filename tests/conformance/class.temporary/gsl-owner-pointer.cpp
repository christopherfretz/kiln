// [class.temporary]/4, clang's [[gsl::Owner]] / [[gsl::Pointer]] (WP-674):
// a Pointer class constructed from (or assigned) a temporary Owner, and a
// pointer or reference taken from a temporary Owner by a tracked std-named
// member, dangles at the end of the full-expression (-Wdangling-gsl,
// -Wdangling-assignment-gsl); returning a Pointer to a local Owner is
// -Wreturn-stack-address, and a member Pointer initialized from a
// temporary or parameter Owner is -Wdangling-field. Explicit attributes on
// classes outside std, so no headers. clang 19's analysis.
//@ strict-diags
//@ std: c++23
//@ requires: clang<22
//@ diag: 34:11 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 35:12 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 38:10 warning "object backing the pointer will be destroyed at the end of the full-expression"
//@ diag: 45:21 warning "returning address of local temporary object"
//@ diag: 46:28 warning "address of stack memory associated with local variable 'o' returned"
//@ diag: 47:26 warning "address of stack memory associated with parameter 'o' returned"
//@ diag: 51:11 warning "initializing pointer member 'p' to point to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 50:7 note "pointer member declared here"
struct [[gsl::Owner(int)]] Own {
  Own();
  ~Own();
  int *begin();
  int &operator*();
  int *data();
};
struct [[gsl::Pointer(int)]] Ptr {
  Ptr();
  Ptr(const Own &);
  int *get();
};
Own make();
void f() {
  Ptr p = Own();
  Ptr p2 = make();
  Own o;
  Ptr ok = o;
  Ptr p3(Own{});
  Ptr p4;
  p4 = Own();
  int *ip = make().data();
  int &ir = *make();
  (void)p; (void)p2; (void)ok; (void)p3; (void)ip; (void)ir;
}
Ptr ret1() { return Own(); }
Ptr ret2() { Own o; return o; }
Ptr ret3(Own o) { return o; }
Ptr ret4(const Own &o) { return o; }
struct M {
  Ptr p;
  M() : p(Own()) {}
  M(Own o) : p(o) {}
};
