// The written-reference table (spec/ast.md §18.5, §18.12.2): `this->`
// names in a class template with a dependent base. Class member lookup
// ([class.member.lookup]/2) returns the declaration set of the class
// itself when it declares the name, before any base, dependent or not,
// so that result is final at the definition ([temp.dep.type]/6): a member
// variable template the class declares (`this->mw<T>`,
// `this->template mw<T>`, also in a nested class of the class template)
// names its template, and it hides a dependent base's member of the same
// name; a using-declaration of a non-dependent base's member is a
// declaration of the class (`found=` its shadow). An overload set the
// class declares (a member function template, overloaded functions;
// `this->`, qualified or unqualified) keeps its found set, closed (no
// `+base`). A data member and a static member are resolved. The name
// stays dependent when the class does not declare it and a dependent
// base could supply it: a using-declaration of a dependent base's member
// (`+base+using`), an overload set found in a non-dependent base
// (`+base`).
//@ written-refs: this-member-dependent-base.refs
struct P {
  template <class U> static constexpr int pw = 3;
  static int pg(int) { return 0; }
  static int pg(long) { return 0; }
};
template <class T> struct B {
  template <class U> static constexpr int mw = 5;
  template <class U> static constexpr int bw = 6;
  template <class U> static int mf() { return 1; }
};
template <class T> struct R : B<T>, P {
  template <class U> static constexpr int mw = 0;
  template <class U> static int mf() { return 0; }
  static int g(int) { return 0; }
  static int g(long) { return 0; }
  int x = 0;
  static constexpr int s = 0;
  using B<T>::bw;
  using P::pw;
  int a() { return this->mw<T> + this->template mw<T> + this->template bw<T> + this->pw<T>; }
  int b() { return this->x + this->s + this->template mf<T>() + this->g(T()) + this->pg(T()); }
  int c() { return g(T()) + R::g(T()) + R::template mf<T>(); }
  struct N : B<T> {
    template <class U> static constexpr int nw = 0;
    int d() { return this->template nw<T> + this->nw<T>; }
  };
};
int z = R<int>().a() + R<int>().b() + R<int>().c() + R<int>::N().d();
