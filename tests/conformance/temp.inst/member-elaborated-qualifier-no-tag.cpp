// [temp.dep.type]/6, [dcl.type.elab]/3, [temp.inst]/3: `class-key B<T>::M`
// written in another class template is a member of an unknown
// specialization even where `B<T>`'s arguments happen to be `B`'s own
// template parameters (same depth and index): the tag is looked up when
// the enclosing class is instantiated, and a specialization without it is
// an error then, for every kind of member declaration (data member,
// parameter, return type, typedef, alias, static data member, enum, friend,
// member template, constructor, conversion function, nested class). kiln
// used to resolve `B<T>` to the primary's pattern and drop the member
// silently when the argument selected an explicit or partial
// specialization (WP-1054; clang dcl.type.elab/p3.cpp). Diagnostics as
// clang 19 and 22. The valid twin is
// member-elaborated-qualifier-specialization-run.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:47 error "no class named 'Member' in 'B<int>'"
//@ diag: 59:12 note "in instantiation of template class 'Field<int>' requested here"
//@ diag: 46:54 error "no class named 'Member' in 'B<int>'"
//@ diag: 60:12 note "in instantiation of template class 'Param<int>' requested here"
//@ diag: 47:45 error "no class named 'Member' in 'B<int>'"
//@ diag: 61:10 note "in instantiation of template class 'Ret<int>' requested here"
//@ diag: 48:57 error "no class named 'Member' in 'B<int>'"
//@ diag: 62:14 note "in instantiation of template class 'Typedef<int>' requested here"
//@ diag: 49:60 error "no class named 'Member' in 'B<int>'"
//@ diag: 63:12 note "in instantiation of template class 'Alias<int>' requested here"
//@ diag: 50:55 error "no class named 'Member' in 'B<int>'"
//@ diag: 64:13 note "in instantiation of template class 'Static<int>' requested here"
//@ diag: 51:51 error "no enum named 'En' in 'B<int>'"
//@ diag: 65:17 note "in instantiation of template class 'EnumMember<int>' requested here"
//@ diag: 52:55 error "no class named 'Member' in 'B<int>'"
//@ diag: 66:13 note "in instantiation of template class 'Friend<int>' requested here"
//@ diag: 54:44 error "no class named 'Member' in 'B<int>'"
//@ diag: 67:21 note "in instantiation of template class 'MemberTemplate<int>' requested here"
//@ diag: 56:51 error "no class named 'Member' in 'B<int>'"
//@ diag: 68:10 note "in instantiation of template class 'Ctor<int>' requested here"
//@ diag: 57:55 error "no class named 'Member' in 'B<int>'"
//@ diag: 69:11 note "in instantiation of template class 'Conv<int>' requested here"
//@ diag: 58:63 error "no class named 'Member' in 'B<int>'"
//@ diag: 70:20 note "in instantiation of member class 'Outer<int>::Nested' requested here"
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
Field<int> f1;
Param<int> p1;
Ret<int> r1;
Typedef<int> t1;
Alias<int> a1;
Static<int> s1;
EnumMember<int> e1;
Friend<int> fr1;
MemberTemplate<int> mt1;
int c1 = sizeof(Ctor<int>);
Conv<int> cv1;
Outer<int>::Nested n1;
