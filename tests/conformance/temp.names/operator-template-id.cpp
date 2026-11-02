// [temp.names]/1: an operator-function-id is a template-name when it names
// a function template, so `operator()<T>` is a template-id: unqualified in
// a member function, qualified (`S::operator()<short>`, `Q<T>::template
// operator()<char>`), after `.template` on a generic lambda with an
// explicit object parameter, and as the operand of `&`. [temp.names]/3
// (P0846): an unqualified operator-function-id that lookup finds nothing
// for, followed by `<`, is assumed to be a template-name and the call
// finds the template by argument-dependent lookup.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ stdout: 206 107 50 80 18
extern "C" int printf(const char*, ...);
namespace N {
struct X { int v; };
template <int K> int operator+(X a, X b) { return a.v + b.v + K; }
}
struct S {
  int base = 100;
  template <class T> int operator()(T t) const { return base + (int)sizeof(T) + t; }
  int g() const { return operator()<char>(1) + S::operator()<short>(2); }
};
template <class T> struct Q {
  template <class U> static int operator()() { return (int)sizeof(U) * 10; }
  static int h() { return Q::operator()<T>() + Q<T>::template operator()<char>(); }
  int k() { return operator()<T>(); }
};

constexpr auto fib = []<int N>(this auto const& self) -> int {
  if constexpr (N < 2) return N;
  else return self.template operator()<N - 1>() + self.template operator()<N - 2>();
};
static_assert(fib.operator()<10>() == 55);
template <class L> constexpr int call(const L& l) { return l.template operator()<3>(); }
static_assert(call(fib) == 2);
struct C {
  template <class T> constexpr int operator()() const { return sizeof(T); }
  constexpr int g() const { return operator()<int>() + this->operator()<char>(); }
};
static_assert(C{}.g() == sizeof(int) + 1);
static_assert(C{}.C::operator()<char>() == 1);
template <class T> struct R {
  template <class U> static constexpr int operator()() { return 7; }
  static constexpr int h() { return R::operator()<int>() + R<T>::template operator()<T>(); }
};
static_assert(R<int>::h() == 14);

int main() {
  S s;
  int (S::*pm)(int) const = &S::operator()<int>;
  N::X a{1}, b{2};
  int r = N::operator+<5>(a, b);
  int r2 = operator+<7>(a, b);
  printf("%d %d %d %d %d\n", s.g(), (s.*pm)(3), Q<int>::h(), Q<double>().k(), r + r2);
}
