// [temp.inst]/3, [class.union.anon]: an instantiated unnamed member class
// is linked to the pattern's unnamed class (WP-3273) and stays an unnamed
// type of its enclosing specialization in the Itanium mangling: a function
// template specialized for it names it `NS0_Ut_E` (clang's
// `<unnamed-type-name>`), as before the link. Reduced from real code.
//@ expect: ir
template <class T> struct S {
  struct { T x; } m;
  union { T a; int b; };
  int f() { return m.x + a + b; }
  template <class U> static int g(U u) { return sizeof(u); }
};
template <class T> int use(T t) { return t.m.x; }
template <class T> struct P;
template <class T> struct P<T *> {
  struct { T y; } n;
  template <class U> static int k(U) { return 1; }
};
S<int> s;
int h() {
  P<long *> p{{5}};
  return s.f() + S<int>::g(s.m) + use(s) + S<long>::g(S<long>().m) +
         P<long *>::k(p.n);
}
