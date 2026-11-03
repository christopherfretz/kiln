//@ args: -fno-exceptions
// WP-432: with `-fno-exceptions` every thunk is `nounwind`
// (`thunk_attrs.cpp` is the unwinding variant).

struct A { virtual ~A(); virtual A *f(); virtual A *h() noexcept; virtual void g() noexcept; };
struct B { virtual ~B(); virtual B *f(); virtual B *h() noexcept; virtual void g() noexcept; int b; };
struct C : A, B {
  ~C();
  C *f() override;
  C *h() noexcept override;
  void g() noexcept override;
};
C *C::f() { return this; }
C *C::h() noexcept { return this; }
void C::g() noexcept {}
C::~C() {}

// The covariant thunks of a virtual base adjust through the vtable.
struct V { virtual V *v(); };
struct D : virtual V { D *v() override; };
D *D::v() { return this; }
