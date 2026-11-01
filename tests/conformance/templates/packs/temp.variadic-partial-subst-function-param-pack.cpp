// [temp.variadic]/5, [temp.inst]/3: a member function template whose
// parameter pack `P<T, U>... ps` names a pack of the class (expanded by
// the class's arguments) and a pack of its own: instantiating the class
// keeps one pack parameter of type `P<T, U>...`, matched element-wise by
// deduction when the member is called.
//@ expect: run
template <class A, class B> struct P { A a; B b; };
template <class... T> struct S {
  template <class... U> static int f(P<T, U>... ps) { return sizeof...(ps); }
  template <class... U> static int h(U... u) { return k(P<T, U>{}...); }
  static int k(...) { return 1; }
};
template struct S<int, char>;
template <int... I> struct N {
  template <class... T> static int call(T... t) { return f2((t + I)...); }
  template <class... T> static int f2(T... t) { return (0 + ... + t); }
};
int main() {
  int r = S<int, char>::f(P<int, long>{}, P<char, short>{});
  if (r != 2) return 1;
  if (S<int, char>::h(1, 2) != 1) return 2;
  if (N<1, 2>::call(10, 20) != 33) return 3;
  return 0;
}
