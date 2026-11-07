// [temp.variadic]/7, [dcl.init.general]/16.4: a parenthesized initializer
// that is an empty pack expansion (`T x(a...)` with `a` empty) is `()`,
// so the variable is value-initialized: zero for a scalar, a pointer or an
// enumeration, the default constructor for a class. Constant evaluation
// sees the same value (Boost.Variant2 `variant::emplace<I>()`).
//@ expect: run
//@ stdout: 0 0 1 0 7 5
extern "C" int printf(const char*, ...);

enum E { e0, e1 };
struct C {
  int v = 7;
};
template <class T, class... A> constexpr T make(A... a) {
  T t(a...);
  return t;
}
template <class... A> int scalar(A... a) {
  int x(a...);
  int* p(a...);
  return x + (p == nullptr);
}
static_assert(make<int>() == 0);
static_assert(make<int>(3) == 3);

int main() {
  printf("%d %d %d %d %d %d\n", make<int>(), (int)make<E>(), scalar(),
         (int)(make<double>() != 0.0), make<C>().v, make<int>(5));
}
