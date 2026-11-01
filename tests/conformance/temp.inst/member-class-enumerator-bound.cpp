// [temp.inst]/3, [temp.dep.constexpr]: a member class's array bound naming an enumerator of the
// enclosing class template is the enumerator of the instantiation; a dependent divisor is not
// diagnosed as division by zero in the template definition
//@ expect: compile-ok
//@ strict-diags
template <class C> struct str {
  enum E { k = 7 };
  enum { m = 10 / sizeof(C) };
  struct B { int d[k]; };
  struct D { C d[m]; };
  int f() { return k; }
};
str<char>::B b;
static_assert(sizeof(b.d) == 7 * sizeof(int));
static_assert(sizeof(str<char>::D) == 10 && sizeof(str<short>::D) == 10);
