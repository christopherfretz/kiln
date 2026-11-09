// [temp.dep.type]/6, [class.member.lookup]/6, [class.access]/4: in
// `class B<T>::In::Deep`, `In` names a member of an unknown specialization,
// looked up again in each specialization: ambiguous in B<long> (two
// bases), inaccessible in B<short> (WP-1071). g++ 14 reports both.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 19 and 22 accept, never looking `In` up again in the specialization
//@ diag: 20:43 error "member 'In' found in multiple base classes of different types"
//@ diag: 21:9 note "in instantiation of template class 'X<long>' requested here"
//@ diag: 15:20 note "member type 'P1::In' found by ambiguous name lookup"
//@ diag: 16:20 note "member type 'P2::In' found by ambiguous name lookup"
//@ diag: 20:43 error "'In' is a private member of 'B<short>'"
//@ diag: 22:10 note "in instantiation of template class 'X<short>' requested here"
//@ diag: 19:47 note "declared private here"
struct P1 { struct In { struct Deep { int y; }; }; };
struct P2 { struct In { struct Deep { int y; }; }; };
template <class T> struct B { struct In { struct Deep { int y; }; }; };
template <> struct B<long> : P1, P2 { };
template <> struct B<short> { private: struct In { struct Deep { int y; }; }; };
template <class T> struct X { class B<T>::In::Deep *d; };
X<long> xl;
X<short> xs;
