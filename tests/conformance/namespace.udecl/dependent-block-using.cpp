// [namespace.udecl]/8, [temp.dep]: a block-scope using-declarator with a
// dependent qualifier (an enumerator, `using E<T>::A;`) is a local
// UnresolvedUsingValueDecl; the instantiation redoes it. Inside a generic
// lambda of a function template it stays dependent until the lambda's own
// specialization, as do a local class's using-declarators. WP-179.
//@ expect: compile-ok

template<class T> struct E { enum Kind { A = 3, B = 4 }; };
template<class T> constexpr int f() {
  using E<T>::B;
  return B;
}
static_assert(f<int>() == 4);

template<class T> constexpr int g() {
  auto l = [](auto x) {
    using E<decltype(x)>::A;
    return A + sizeof(T);
  };
  return l(0);
}
static_assert(g<char>() == 4);

template<class T> struct W { enum { Z = 9 }; };
struct M { int notenum = 6; };
template<class T> struct Outer {
  template<class U> struct In : U {
    using U::notenum;
    constexpr int get() const { return notenum; }
  };
  template<class U> static constexpr int h() {
    using W<U>::Z;
    return Z;
  }
};
static_assert(Outer<int>::h<long>() == 9);
static_assert(Outer<int>::In<M>().get() == 6);

struct B { int v = 5; using vt = int; };
template<class T> constexpr int k() {
  auto l = [](auto x) {
    struct L : decltype(x) {
      using decltype(x)::v;
      using typename decltype(x)::vt;
      constexpr vt get() const { return v; }
    };
    return L().get() + sizeof(T);
  };
  return l(B());
}
static_assert(k<short>() == 7);
