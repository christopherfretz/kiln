// [temp.variadic]/5, [dcl.fct]/5: a function parameter pack whose array
// bound alone names the pack (`int ...a[Ns]`) expands the unadjusted
// array type; each element is then adjusted to a pointer.
//@ expect: run
//@ std: c++20
template <int... Ns> constexpr int f(int ...a[Ns]) { return (a[0] + ... + 0); }
template <int... Ns> struct X {
  static constexpr int g(int ...a[Ns]) { return sizeof...(a); }
};
int main() {
  int x[1] = {3}, y[2] = {4, 5};
  if (f<1, 2>(x, y) != 7) return 1;
  int *p = x;
  if (f<9>(p) != 3) return 2;
  if (X<1, 2, 3>::g(x, y, y) != 3) return 3;
  return 0;
}
