// [temp.deduct.type]/10: if the function parameter associated with A is a
// function parameter pack and the one associated with P is not, deduction
// fails. So in partial ordering ([temp.deduct.partial], [temp.spec.partial.order],
// [temp.func.order]) `R(A)` is more specialized than `R(A...)`: a trait over a
// unary call operator picks the single-parameter specialization, not
// "ambiguous partial specializations".
//@ expect: run
//@ strict-diags
template <class L> struct T : T<decltype(&L::operator())> {};
template <class C, class R, class... A> struct T<R (C::*)(A...) const> {
  static constexpr int v = 1;
};
template <class C, class R, class A> struct T<R (C::*)(A) const> {
  static constexpr int v = 2;
};
template <class F> struct P;
template <class R, class... A> struct P<R (*)(A...)> { static constexpr int v = 1; };
template <class R, class A> struct P<R (*)(A)> { static constexpr int v = 2; };
template <class R, class A, class... B> struct P<R (*)(A, B...)> { static constexpr int v = 3; };
template <class F> struct Q;
template <class R, class... A> struct Q<R(A...) const> { static constexpr int v = 1; };
template <class R, class A> struct Q<R(A) const> { static constexpr int v = 2; };

template <class R, class... A> int f(R (*)(A...)) { return 1; }
template <class R, class A> int f(R (*)(A)) { return 2; }

int g(bool);
int h(bool, int);
int k();
auto l = [](bool) { return 1; };
auto m = [](bool, int) { return 1; };
static_assert(T<decltype(l)>::v == 2);
static_assert(T<decltype(m)>::v == 1);
static_assert(P<int (*)(bool)>::v == 2);
static_assert(P<int (*)(bool, int)>::v == 3);
static_assert(P<int (*)()>::v == 1);
static_assert(Q<int(bool) const>::v == 2);

int g(bool) { return 0; }
int h(bool, int) { return 0; }
int k() { return 0; }
int main() { return f(g) * 100 + f(h) * 10 + f(k) - 211; }
