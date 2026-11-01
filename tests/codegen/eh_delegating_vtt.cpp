// [except.ctor]/4: once the target of a delegating constructor returns,
// a throw from the delegating body destroys the object. In a base-object
// constructor of a class with virtual bases the EH cleanup calls the
// base-object destructor with the current VTT parameter, unadjusted
// (clang `CallDelegatingCtorDtor`, `GetVTTParameter(Delegating)`).
void g();
struct V { V(); ~V(); int v; };
struct W { W(); ~W(); };
struct A : virtual V { A(); ~A(); };
struct M { M(); ~M(); };
struct B : A, virtual W { M m; B(int, int); B(int); B(); ~B(); virtual void f(); };
B::B(int, int) {}
B::B(int a) : B(a, a) { g(); }
B::B() : B(1) { g(); }
struct D : B { D(); D(int); };
D::D(int) : B() {}
D::D() : D(0) { g(); }
B *mk() { return new B; }
