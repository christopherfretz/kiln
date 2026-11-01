// [expr.mptr.oper]/6: a `&&`-qualified member function is called through
// a pointer to member only on an rvalue, a `&`-qualified one only on an
// lvalue (or, since C++20, an rvalue when it is `const &`); the object is
// no more cv-qualified than the function. Both are SFINAE errors, which
// is how std::is_invocable rejects these cases.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:7 error "pointer-to-member function type 'LR' (aka 'void (S::*)() &') can only be called on an lvalue"
//@ diag: 21:17 error "call to pointer to member function of type 'void () volatile' drops 'const' qualifier"
//@ diag: 22:17 error "call to pointer to member function of type 'int (int)' drops 'const volatile' qualifiers"
//@ diag: 23:5 error "pointer-to-member function type 'RR' (aka 'void (S::*)() &&') can only be called on an rvalue"
//@ diag: 24:6 error "pointer-to-member function type 'RR' (aka 'void (S::*)() &&') can only be called on an rvalue"
struct S { void f() const &; void g() &; void h() volatile; int m(int); void r() &&; };
using CR = void (S::*)() const &;
using LR = void (S::*)() &;
using RR = void (S::*)() &&;
void t(S s, const volatile S* cv) {
  (S().*CR(&S::f))();
  (static_cast<S&&>(s).*RR(&S::r))();
  (S().*LR(&S::g))();
  (cv->*(&S::h))();
  (cv->*(&S::m))(1);
  (s.*RR(&S::r))();
  (&s->*RR(&S::r))();
}

template <class T> T&& declval();
template <class F, class A, class = decltype((declval<A>().*declval<F>())())>
constexpr bool ok(int) { return true; }
template <class F, class A> constexpr bool ok(...) { return false; }
static_assert(ok<RR, S>(0) && ok<RR, S&&>(0) && !ok<RR, S&>(0));
static_assert(ok<LR, S&>(0) && !ok<LR, S>(0) && ok<CR, S>(0));
static_assert(!ok<void (S::*)(), const S&>(0) && ok<void (S::*)() const, const S&>(0));
