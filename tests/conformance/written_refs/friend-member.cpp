// The written-reference table (spec/ast.md §18.4): a friend that is a
// member of every specialization of a class template ([temp.friend]/5,
// `template <class T> friend void S<T>::f();`) names the member of the
// class pattern its qualifier names, as an out-of-line member's
// qualifier names one ([temp.spec.partial.member]/1): `S<T>` with the
// friend's own parameters the primary template's, `S<T *>` the partial
// specialization's own member, a different entity. Member classes on
// the way are followed (`S<T>::In::g`), and among overloads the friend's
// type picks one. A qualifier that names no single pattern (`S2<T,
// int>`) or a dependent class (`friend void T::f();`) leaves the name
// dependent.
//@ written-refs: friend-member.refs
namespace n {
template <class T> struct S {
  void f();
  void h(int);
  void h(long);
  struct In {
    void g();
  };
};
template <class T> struct S<T *> {
  void f();
};
template <class T, class U> struct S2 {
  void f();
};
} // namespace n

class X {
  template <class T> friend void n::S<T>::f();
  template <class T> friend void n::S<T *>::f();
  template <class U> friend void n::S<U>::h(long);
  template <class T> friend void n::S<T>::In::g();
  template <class T> friend void n::S2<T, int>::f();
};

template <class V> class Y {
  template <class T> friend void n::S<T>::f();
  friend void V::f();
};
Y<n::S<int>> y;
