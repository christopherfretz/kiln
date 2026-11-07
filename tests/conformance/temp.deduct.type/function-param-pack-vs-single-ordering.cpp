// [temp.deduct.type]/10, [temp.spec.partial.order], [temp.func.order]: in
// partial ordering, a function parameter pack `A...` of a function type's
// parameter list does not deduce a non-pack parameter `A`, so the
// single-parameter form `R (C::*)(A) const` is more specialized than
// `R (C::*)(A...) const` (and `void(*)(A)` than `void(*)(A...)` for
// function templates). Reduced from Catch2 v3.5.0's clara
// UnaryLambdaTraits.
//@ expect: run
//@ exit: 0
template <class L> struct Traits : Traits<decltype(&L::operator())> {};
template <class C, class R, class... A> struct Traits<R (C::*)(A...) const> {
  static const int arity = -1;
};
template <class C, class R, class A> struct Traits<R (C::*)(A) const> {
  static const int arity = 1;
};

template <class L> struct Fn;
template <class... A> struct Fn<void(A...)> { static const int v = 2; };
template <class A> struct Fn<void(A)> { static const int v = 1; };
template <class A> struct Fn<void(A, ...)> { static const int v = 3; };

template <class... A> int f(void (*)(A...)) { return 2; }
template <class A> int f(void (*)(A)) { return 1; }

void g(int) {}
void h(int, int) {}

int main() {
  auto unary = [](bool) { return 1; };
  auto binary = [](bool, int) { return 2; };
  static_assert(Traits<decltype(unary)>::arity == 1);
  static_assert(Traits<decltype(binary)>::arity == -1);
  static_assert(Fn<void(int)>::v == 1);
  static_assert(Fn<void()>::v == 2);
  static_assert(Fn<void(int, int)>::v == 2);
  static_assert(Fn<void(int, ...)>::v == 3);
  return f(g) == 1 && f(h) == 2 ? 0 : 1;
}
