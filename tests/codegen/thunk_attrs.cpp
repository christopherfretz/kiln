// WP-432: a thunk is `nounwind` only by its method's type or attributes
// (clang `ConstructAttributeList(IsThunk)`), never by inference over its
// body: the covariant thunk of a non-`noexcept` method stays unwinding
// although everything it calls is `nounwind`. `thunk_attrs_noexc.cpp` is
// the `-fno-exceptions` variant.

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
