// [temp.expl.spec]/2 (an explicit specialization at class scope),
// [temp.dep.constexpr]/2: an argument whose operand depends on the
// enclosing template's parameter (`sizeof(sizeof(g(T())))`) is
// instantiation-dependent but not value-dependent; it is `sizeof(size_t)`
// in every instantiation. Kiln keeps it as written (spec/ast.md 8.3
// "Arguments as written") and matches it by its value. Reduced from real
// code.
//@ expect: run
//@ stdout: 1 0 1
extern "C" int printf(const char *, ...);

int g(...);

template <class T> struct S {
  template <unsigned long N> int h() { return 0; }
  template <> int h<sizeof(sizeof(g(T())))>() { return 1; }
};

int main() {
  S<int> si;
  S<char> sc;
  printf("%d %d %d\n", si.h<sizeof(sizeof(0))>(), si.h<1>(),
         sc.h<sizeof(unsigned long)>());
}
