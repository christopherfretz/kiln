// [except.ctor]/3 for list-initialized arrays: the elements constructed
// so far are destroyed when a later initializer throws. clang's
// `EmitNewArrayInitializer` / `EmitArrayInit` push an irregular
// partial-array EH cleanup that reads `array.init.end`; when nothing in
// the initializer can throw, the cleanup is never emitted and its
// auxiliary alloca and stores are dropped (`AddAuxAllocas`).
namespace std { struct nothrow_t {}; extern const nothrow_t nothrow; }
void* operator new[](unsigned long, const std::nothrow_t&) noexcept;
int f();
struct C { C(); ~C(); int k; };
struct E { ~E(); int k; };
struct G { ~G(); int k = f(); };

// Explicit elements, then a constructor-call filler loop.
C* b1(int n) { return new C[n]{C()}; }
C* b2() { return new C[3]{C(), C()}; }
C* b5(int n) { return new C[n]{}; }
C* a12() { return new C[2]{}; }
C* b8(int n) { return new C[n]{C(), C()}; }

// Aggregate elements: no cleanup unless an initializer can throw.
E* b3(int n) { return new E[n]{{1}, {2}}; }
E* b4() { return new E[2]{}; }
E* b6(int n) { return new E[n]{{f()}, {2}}; }
E* b7() { return new E[2]{{f()}}; }

// A generic element loop for the filler.
G* e1(int n) { return new G[n]{{1}}; }
G* e2() { return new G[3]{{1}}; }
G (*e7(int n))[2] { return new G[n][2]{}; }

// Multidimensional allocations destroy base elements.
C (*e3(int n))[2] { return new C[n][2]{{C(), C()}}; }
E (*e6(int n))[2] { return new E[n][2]{{{f()}, {2}}}; }

// Inside the null check of a nothrow new the first element is saved.
C* e4(int n) { return new (std::nothrow) C[n]{C()}; }

// Local arrays.
void g1() { E a[2]{{1}, {2}}; }
void g2() { E a[2]{{f()}, {2}}; }
