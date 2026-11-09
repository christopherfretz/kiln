// [class.union.general]/2: a union has no virtual functions; a virtual
// destructor, a pure virtual function and a virtual member template are
// each diagnosed once, at 'virtual' (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "unions cannot have virtual functions"
//@ diag: 10:27 error "unions cannot have virtual functions"
//@ diag: 11:30 error "unions cannot have virtual functions"
union U1 { virtual ~U1(); virtual void g() = 0; };
union U2 { template<class T> virtual void f(); };
union U3 { void f(); static void g(); int a; };
