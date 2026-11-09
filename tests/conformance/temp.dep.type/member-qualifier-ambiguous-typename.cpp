// [class.member.lookup]/6, [class.access]/4, [temp.res.general]/3: in
// `typename B<T>::In::Deep`, `In` is looked up in each specialization:
// found in two bases (B<long>) it is ambiguous, and a private member
// (B<short>) is inaccessible. kiln accepted the ambiguous qualifier (WP-1071).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:46 error "member 'In' found in multiple base classes of different types"
//@ diag: 20:9 note "in instantiation of template class 'X<long>' requested here"
//@ diag: 14:20 note "member type 'P1::In' found by ambiguous name lookup"
//@ diag: 15:20 note "member type 'P2::In' found by ambiguous name lookup"
//@ diag: 19:46 error "'In' is a private member of 'B<short>'"
//@ diag: 21:10 note "in instantiation of template class 'X<short>' requested here"
//@ diag: 18:47 note "declared private here"
struct P1 { struct In { struct Deep { int y; }; }; };
struct P2 { struct In { struct Deep { int y; }; }; };
template <class T> struct B { struct In { struct Deep { int y; }; }; };
template <> struct B<long> : P1, P2 { };
template <> struct B<short> { private: struct In { struct Deep { int y; }; }; };
template <class T> struct X { typename B<T>::In::Deep *d; };
X<long> xl;
X<short> xs;
