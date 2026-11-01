// [temp.expl.spec]/16: `template<> template<class U> R A<long>::g(U)`
// specializes member template `g` of the implicit specialization
// `A<long>` (still a template); its specializations instantiate from the
// new definition, other `A<T>` keep the pattern's. Declared first and
// defined after a use, static, and with the enclosing argument
// substituted in the signature.
//@ expect: run
//@ stdout: 2 6 1 7 5 4 3 3
extern "C" int printf(const char *, ...);

template <class T> struct A {
  template <class U> constexpr int g(U) const { return 1; }
  template <class U> static int h(U);
  template <class U> int k(U, T) { return 3; }
};
template <> template <class U> constexpr int A<long>::g(U) const;
int use() { return A<long>().g(1); }
template <> template <class U> constexpr int A<long>::g(U) const {
  return 2 + int(sizeof(U));
}
template <> template <class U> int A<char>::h(U) { return 7; }
template <class T> template <class U> int A<T>::h(U) { return 5; }
template <> template <class U> int A<short>::k(U, short) { return 4; }

static_assert(A<long>().g('c') == 3);
static_assert(A<int>().g('c') == 1);

int main() {
  A<short> s;
  A<int> b;
  printf("%d %d %d %d %d %d %d %d\n", A<long>().g('c') - 1, use(), b.g(1),
         A<char>::h(1), A<int>::h(1), s.k(1, 1), b.k(1, 1), A<long>().k(1, 1L));
}
