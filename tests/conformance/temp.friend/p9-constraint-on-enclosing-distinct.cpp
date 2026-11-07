//@ expect: run
//@ stdout: 3 4 1 2 10 20
// [temp.friend]/9: a friend function template whose constraint depends
// on a template parameter of the enclosing class template (here through
// the injected-class-name or `T`) declares a different template in each
// specialization: W<int> and W<long> each define their own `get`, with
// the same signature and default template argument. A friend whose
// constraints name only its own parameters still redeclares the
// namespace-scope template.
#include <cstdio>
#include <type_traits>

template <class T> struct W {
  T v;
  template <typename Self, typename T2 = T>
    requires(std::is_same_v<std::remove_cvref_t<Self>, W>)
  friend T2 get(Self&& s) { return s.v; }
};

template <class T> struct S {
  template <class U>
    requires(sizeof(T) >= 1)
  friend int tag(S, U u) { return static_cast<int>(u); }
};

template <class U>
  requires std::is_integral_v<U>
int twice(U u);

template <class T> struct R {
  template <class U>
    requires std::is_integral_v<U>
  friend int twice(U u);
};

template <class U>
  requires std::is_integral_v<U>
int twice(U u) { return 2 * static_cast<int>(u); }

int main() {
  W<int> a{3};
  W<long> b{4};
  S<char> c;
  S<double> d;
  R<int> r1;
  R<long> r2;
  (void)r1;
  (void)r2;
  std::printf("%d %ld %d %d %d %d\n", get(a), get(b), tag(c, 1), tag(d, 2), twice(5), twice(10L));
}
