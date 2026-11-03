// [temp.point]/1, /7, [temp.inst]/7: a variable template specialization
// named before its template's (or its static data member's) definition is
// instantiated from the definition seen since: at a later reference, whose
// value a constant expression can then use (clang
// `InstantiateVariableDefinition` finds the definition there), or at the
// end of the translation unit for one that cannot be constant (GCC defines
// neither).
//@ expect: run
//@ skip(gcc): GCC 16 leaves e<int> and m<short> undefined (a link error)
//@ stdout: 12 102 15 7
extern "C" int printf(const char*, ...);
int dyn() {
  static int n;
  return ++n * 10;
}

template <class T> extern const int e;
int ge() { return e<int>; }
template <class T> const int e = sizeof(T) * 3;
static_assert(e<int> == 12);
int ge2() { return e<int>; }

template <class T> extern int m;
int gm() { return m<short>; }
template <class T> int m = sizeof(T) + 100;

template <class T> struct S {
  static const int s;
  static int t;
};
int gs() { return S<int>::s + S<int>::t; }
template <class T> const int S<T>::s = 5;
template <class T> int S<T>::t = dyn();
static_assert(S<int>::s == 5);

template <class T> constexpr T cv = T(7);
constexpr int use_cv = cv<int>;

int main() { printf("%d %d %d %d\n", ge() + ge2() - 12, gm(), gs(), use_cv); }
