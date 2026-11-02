// [class.base.init]/2, [temp.inst]: a mem-initializer that fails when the
// constructor is instantiated makes the specialization invalid (clang's
// InstantiateMemInitializers), so a constant evaluation calling it fails
// with no evaluation note ([expr.const]/5), and the invalid constructor
// is no longer a candidate ([over.match.funcs]).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 24:29 error "constructor initializer 'decltype(t)' (aka 'const B &') does not name a class"
//@ diag: 28:16 note "in instantiation of member function 'E<B>::E' requested here"
//@ diag: 28:16 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:15 note "initializer of 'e' is not a constant expression"
//@ diag: 28:16 note "declared here"
//@ diag: 30:15 error "no matching conversion for functional-style cast from 'B' to 'E<B>'"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'B' to 'const E<B>' for 1st argument"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'B' to 'E<B>' for 1st argument"
struct B {
  int v;
  constexpr B(int x) : v(x) {}
};
template <class T>
struct E : T {
  constexpr E(const T& t) : decltype(t)(t) {}
  constexpr int get() const { return this->v; }
};
constexpr B b(5);
constexpr E<B> e(b);
static_assert(e.v == 5);
static_assert(E<B>(B(5)).get() == 5);
