// [expr.assign]/8, [temp.dep.expr]/3: `x = {...}` with a type-dependent
// left operand or list element is dependent and assigned from the list
// at instantiation (libc++ split_view's `__next_ = {__cur_, __cur_}`).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:62 error "excess elements in scalar initializer"
//@ diag: 32:17 note "in instantiation of member function 'Bad<int>::g' requested here"
template <class I> struct SR {
  constexpr SR() = default;
  constexpr SR(I a, I b) : a(a), b(b) {}
  I a{}, b{};
};
struct P { int a, b; };

template <class V> struct It {
  V c = V(2);
  SR<V> n;
  P p{};
  int k = 0;
  constexpr int g() {
    n = {c, c};
    p = {c, 3};
    k = {V(1)};
    k += {c};
    return n.a + n.b + p.a + p.b + k;
  }
};
static_assert(It<int>{}.g() == 12);

template <class V> struct Bad { V *x; void g() { V v{}; *x = {v, v}; } };
template struct Bad<int>;
