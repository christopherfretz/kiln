// [temp.local]/1, [temp.explicit]: after `S<char>::` in a declarator, the
// injected-class-name `S` names the specialization, so `S::I` in the
// parameter list is a type (WP-141: kiln parsed the declaration as a
// variable and rejected `S` as needing template arguments).
//@ expect: compile-ok
//@ strict-diags
template <class C> struct S {
  typedef int I;
  I h(I);
};
template <class C> typename S<C>::I S<C>::h(S::I) { return 0; }
extern template S<char>::I S<char>::h(S::I);
template <class C> struct Al {};
template <class C, class A = Al<C>> struct S2 {
  typedef int I;
  I h(I, C);
};
extern template S2<char>::I S2<char>::h(S2::I, char);
struct T {
  typedef int I;
  I h(I);
};
T::I T::h(T::I) { return 0; }
