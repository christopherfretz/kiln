// `::delete` of a polymorphic class: the complete object is found through
// the offset-to-top word, the complete destructor is called virtually, and
// the global (sized) `operator delete` frees it, even when the class has its
// own `operator delete`. Final classes are devirtualized.
using size_t = decltype(sizeof 0);
struct A { int a; virtual ~A(); };
struct B { long b; virtual void g(); virtual ~B() {} };
struct C : B { int c; ~C() override {} };
struct D { int d; virtual ~D() {} void operator delete(void*, size_t) noexcept; };
struct E : A, D { int e; };
struct F final : B { int f; };
struct alignas(32) G { int g; virtual ~G() {} };
struct H : virtual B { int h; };
void f(A* p) noexcept { ::delete p; }
void g(B* p) noexcept { ::delete p; }
void h(C* p) noexcept { ::delete p; }
void i(D* p) noexcept { ::delete p; }
void j(D* p) noexcept { delete p; }
void k(E* p) noexcept { ::delete p; }
void l(F* p) noexcept { ::delete p; }
void m(G* p) noexcept { ::delete p; }
void n(H* p) noexcept { ::delete p; }
