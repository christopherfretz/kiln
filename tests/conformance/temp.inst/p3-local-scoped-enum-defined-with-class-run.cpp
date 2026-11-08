// [temp.inst]/3, CWG1484: a scoped member enumeration of a class template
// is defined on demand, but one declared inside a function template (a
// member of a local class) is defined with its class, so its enumerators
// are found through `L::E::e` and from a member function of `L`, before or
// after the enumeration's declaration (clang `VisitEnumDecl`). WP-884.
//@ expect: run
//@ stdout: 8 8 8 4 12
extern "C" int printf(const char *, ...);

template <class T> constexpr int k1() {
  struct L { enum class E { e = sizeof(T) }; };
  return int(L::E::e);
}
template <class T> constexpr int k2() {
  struct L {
    enum class E { e = sizeof(T) };
    static constexpr int f() { return int(E::e); }
  };
  return L::f();
}
template <class T> constexpr int k3() {
  struct L {
    static constexpr int f() { return int(E::e) + int(sizeof(E)); }
    enum class E : T { e = 8 };
  };
  return L::f();
}
template <class T> int k4() {
  struct L { struct M { enum struct E { a, b, c = sizeof(T) + 4 }; }; };
  return int(L::M::E::c);
}
static_assert(k1<long long>() == 8 && k2<long long>() == 8);
static_assert(k3<int>() == 12);

int main() {
  printf("%d %d %d %d %d\n", k1<long long>(), k2<long long>(), k1<double>(), k3<int>() - 8,
         k4<long long>());
}
