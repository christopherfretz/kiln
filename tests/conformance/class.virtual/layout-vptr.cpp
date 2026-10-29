// [class.virtual]/1-2, [class.derived], Itanium C++ ABI §2.4 (a dynamic class
// without a primary base gets a vptr at offset 0; the first non-virtual
// dynamic base becomes the primary base and shares it), §2.5.
//@ expect: layout
struct A { virtual void f(); int a; };
struct N { int n; };
struct B : N, A { int b; };               // A is primary despite N first
struct C { virtual void g(); char c; };
struct D : A, C { virtual void h(); };    // A primary, C secondary at 16
void A::f() {} void C::g() {} void D::h() {}
D d; B b;
