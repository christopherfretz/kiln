// [temp.dep.type]/6, [dcl.type.elab]/3, [temp.inst]/3: `class-key B<T>::M`
// in another class template, with `B<T>`'s arguments at the positions of
// `B`'s own parameters, names the tag of whichever specialization the
// argument selects: the primary's, an explicit specialization's or a
// partial specialization's. Every kind of member declaration keeps its
// member (kiln used to drop those naming a specialization's tag, so the
// class lost a data member and its layout changed; WP-1054). Valid twin of
// member-elaborated-qualifier-no-tag.cpp and -wrong-tag.cpp.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 8
#include <cstdio>
template <class T> struct B { class Member { public: int v = 1; }; enum En { e = 1 }; };
template <> struct B<long> {
  class Member { public: int v = 2; int w = 0; };
  enum En { e = 3 };
};
template <class T> struct B<T *> { class Member { public: int v = 4; }; };
template <class T> struct Field { class B<T>::Member m; };
template <class T> struct Param { int f(class B<T>::Member x) { return x.v; } };
template <class T> struct Ret { class B<T>::Member f() { return {}; } };
template <class T> struct Typedef { typedef class B<T>::Member type; };
template <class T> struct Alias { using type = class B<T>::Member; };
template <class T> struct Static { static class B<T>::Member s; };
template <class T> class B<T>::Member Static<T>::s;
template <class T> struct EnumMember { enum B<T>::En e; };
class Secret {
  int k = 9;
  template <class> friend struct Friend;
};
template <class T> struct Friend {
  friend class B<T>::Member;
  int get(Secret s) { return s.k; }
};
template <class T> struct MemberTemplate {
  template <class U> int f(U u, class B<T>::Member m) { return u + m.v; }
};
template <class T> struct Ctor {
  int v;
  Ctor(class B<T>::Member m) : v(m.v + 10) {}
};
template <class T> struct Conv {
  operator class B<T>::Member() { return {}; }
};
template <class T> struct Outer { struct Nested { class B<T>::Member m; }; };
int main() {
  Field<int> f1;
  Field<long> f2;
  static_assert(sizeof(Field<long>) == 2 * sizeof(int));
  Param<int *> p;
  Ret<long> r;
  Typedef<long>::type t;
  Alias<char *>::type a;
  EnumMember<long> e{B<long>::e};
  Friend<long> fr;
  MemberTemplate<long> mt;
  Ctor<long> c{B<long>::Member{}};
  B<int *>::Member cv = Conv<int *>();
  Outer<long>::Nested n;
  std::printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", f1.m.v, f2.m.v, e.e,
              p.f(B<int *>::Member{}), r.f().v + 3, t.v + t.w + 4, a.v + 3,
              Static<long>::s.v + 6, fr.get(Secret{}), mt.f(8, B<long>::Member{}),
              Static<int>::s.v + 10, c.v, cv.v + 9, n.m.v + 12, (int)sizeof(f2));
}
