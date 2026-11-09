// [dcl.type.elab]/3, [temp.dep.type]/6, [temp.inst]/3: the tag that
// `class-key B<T>::M` names in another class template is looked up, and its
// kind checked against the class-key, when the enclosing class is
// instantiated: a union, struct or enum of an explicit or partial
// specialization named with `class` (or a class named with `enum`) is an
// error, reported at the class-key, for every kind of member declaration.
// kiln used to drop such a member silently (WP-1054; clang
// dcl.type.elab/p3.cpp:60). Diagnostics as clang 19 and 22. The valid twin
// is member-elaborated-qualifier-specialization-run.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 56:35 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 70:10 note "in instantiation of template class 'Field<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 56:35 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 71:15 note "in instantiation of template class 'Field<char *>' requested here"
//@ diag: 55:41 note "previous use is here"
//@ diag: 57:42 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 72:10 note "in instantiation of template class 'Param<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 58:33 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 73:8 note "in instantiation of template class 'Ret<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 59:45 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 74:12 note "in instantiation of template class 'Typedef<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 60:48 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 75:10 note "in instantiation of template class 'Alias<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 61:43 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 76:11 note "in instantiation of template class 'Static<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 62:40 error "use of 'En' with tag type that does not match previous declaration"
//@ diag: 77:15 note "in instantiation of template class 'EnumMember<A>' requested here"
//@ diag: 54:61 note "previous use is here"
//@ diag: 63:43 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 78:11 note "in instantiation of template class 'Friend<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 65:32 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 79:19 note "in instantiation of template class 'MemberTemplate<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 67:39 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 80:10 note "in instantiation of template class 'Ctor<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 68:43 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 81:9 note "in instantiation of template class 'Conv<A>' requested here"
//@ diag: 54:33 note "previous use is here"
//@ diag: 69:51 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 82:18 note "in instantiation of member class 'Outer<A>::Nested' requested here"
//@ diag: 54:33 note "previous use is here"
struct A {};
template <class T> struct B { class Member {}; enum En { e }; };
template <> struct B<int> {};
template <> struct B<A> { union Member { void *p; }; struct En {}; };
template <class T> struct B<T *> { enum Member { m }; };
template <class T> struct Field { class B<T>::Member *p; };
template <class T> struct Param { void f(class B<T>::Member); };
template <class T> struct Ret { class B<T>::Member *f(); };
template <class T> struct Typedef { typedef class B<T>::Member type; };
template <class T> struct Alias { using type = class B<T>::Member; };
template <class T> struct Static { static class B<T>::Member *s; };
template <class T> struct EnumMember { enum B<T>::En e; };
template <class T> struct Friend { friend class B<T>::Member; };
template <class T> struct MemberTemplate {
  template <class U> void f(U, class B<T>::Member *);
};
template <class T> struct Ctor { Ctor(class B<T>::Member *); };
template <class T> struct Conv { operator class B<T>::Member *(); };
template <class T> struct Outer { struct Nested { class B<T>::Member *p; }; };
Field<A> f2;
Field<char *> f3;
Param<A> p2;
Ret<A> r2;
Typedef<A> t2;
Alias<A> a2;
Static<A> s2;
EnumMember<A> e2;
Friend<A> fr2;
MemberTemplate<A> mt2;
int c2 = sizeof(Ctor<A>);
Conv<A> cv2;
Outer<A>::Nested n2;
