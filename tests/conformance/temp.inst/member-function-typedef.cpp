// [dcl.fct]/13, [temp.inst]/3, [class.mfct]/1: a member function declared
// through a typedef of function type (`F m;`, `G g;` with a
// cv-qualified function typedef, `static F s;`, a dependent `H h;`)
// substitutes when the enclosing class template is instantiated. The
// typedef-declared member gets unnamed parameters synthesized from the
// prototype (clang `BuildParmVarDeclForTypedef`), so an out-of-class
// definition `int S<T>::g(int x) const { return x; }` instantiates with
// its own parameter `x` bound.
//@ expect: run
//@ strict-diags
typedef void F();
typedef int G(int) const;
template <class T> struct S {
  F m;
  T n;
  struct In { F k; static F sk; };
  G g;
  static F s;
  typedef T H(T);
  H h;
};
template <class T> void S<T>::m() {}
template <class T> int S<T>::g(int x) const { return x; }
template <class T> T S<T>::h(T x) { return x + n; }
template <class T> void S<T>::In::k() {}
template <class T> void S<T>::s() {}
int main() {
  S<int> s{};
  s.m();
  S<int>::In i;
  i.k();
  S<int>::s();
  return s.g(1) + s.h(2) - 3;
}
