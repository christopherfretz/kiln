// [except.spec]/6.1, [expr.unary.noexcept]: a call through a pointer to
// member function is potentially-throwing unless the member function type
// is noexcept; the `.*` / `->*` operand is a bound-member prvalue, so the
// specification is read from the pointer's type (libc++
// is_nothrow_invocable<decltype(&Dummy::foo), Dummy&>).
//@ expect: compile-ok
//@ strict-diags
template <class T> T&& declval() noexcept;
struct D { void foo() noexcept {} void bar() {} };
static_assert(noexcept((declval<D&>().*declval<void (D::*)() noexcept>())()));
static_assert(!noexcept((declval<D&>().*declval<void (D::*)()>())()));
static_assert(noexcept((declval<D*>()->*declval<void (D::*)() noexcept>())()));
static_assert(!noexcept((declval<D*>()->*declval<void (D::*)()>())()));
void f() {
  D d; auto p = &D::foo; auto q = &D::bar;
  static_assert(noexcept((d.*p)()));
  static_assert(!noexcept((d.*q)()));
}
