// [dcl.fct.default]/4 with [temp.expl.spec]: an explicit specialization
// without default arguments uses the primary template's and the member's
// (the sibling default-arg-explicit-spec rejects adding one).
//@ std: c++20
//@ expect: run
template <class T> int h(T, int k = 2) { return k; }
template <> int h<int>(int, int k) { return k + 10; }
template <class T> struct S { int f(int x = 1); template <class U> int g(U, int y = 3); };
template <class T> int S<T>::f(int x) { return x; }
template <class T> template <class U> int S<T>::g(U, int y) { return y; }
template <> int S<int>::f(int x) { return x + 20; }
template <> template <> int S<int>::g<int>(int, int y) { return y + 30; }
int main() {
  S<int> s;
  return h(1) == 12 && h('a') == 2 && s.f() == 21 && s.g(1) == 33 ? 0 : 1;
}
