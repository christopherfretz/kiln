// [namespace.udecl]/1, /3, [temp.dep.type]: a class-scope using-declarator
// naming a member of a dependent base (clang's UnresolvedUsingValueDecl /
// UnresolvedUsingTypenameDecl) is redone at instantiation; its uses in
// the class are implicit member accesses or static member references,
// with the class's own overloads joining the base's set, and a
// `using typename` name works as a nested-name-specifier. A member class
// template of a class template keeps its using-declarators dependent
// until its own instantiation. WP-179.
//@ expect: compile-ok

struct B1 {
  int m = 2;
  static constexpr int sf() { return 3; }
  constexpr int vf() const { return 4; }
  static constexpr int sm = 10;
};
template<class T> struct D1 : T {
  using T::m;
  using T::sf;
  using T::vf;
  using T::sm;
  constexpr int g() const { return m + sf() + vf() + sm; }
  static constexpr int s() { return sf() + sm; }
};
static_assert(D1<B1>().g() == 19);
static_assert(D1<B1>::s() == 13);

struct B2 {
  constexpr int f(int) const { return 1; }
  constexpr int f(double) const { return 2; }
};
template<class T> struct D2 : T {
  using T::f;
  constexpr int f(char) const { return 3; }
  constexpr int g() const { return f(1) * 100 + f(1.0) * 10 + f('c'); }
};
static_assert(D2<B2>().g() == 123);

struct V {
  constexpr virtual int v() const { return 1; }
};
template<class T> struct D3 : T {
  using T::v;
  constexpr int call() const { return v(); }
};
struct X : D3<V> {
  constexpr int v() const override { return 7; }
};
static_assert(X().call() == 7);
static_assert(D3<V>().call() == 1);

struct In { static constexpr int value = 5; using vt = long; };
struct B { using type = In; };
template<class T> struct D : T {
  using typename T::type;
  static constexpr int v = type::value;
  typename type::vt w = 1;
  type obj;
  constexpr int get() const { return type::value + sizeof(typename type::vt); }
};
static_assert(D<B>::v == 5);
static_assert(D<B>().get() == 13);
static_assert(sizeof(D<B>().w) == 8);

struct B4 { int v = 5; using vt = int; };
template<class T> struct O {
  template<class U> struct I : U {
    using U::v;
    using typename U::vt;
    vt get() { return v + sizeof(T); }
  };
};
static_assert(sizeof(O<char>::I<B4>) == 4);
int k1 = O<char>::I<B4>().get();
