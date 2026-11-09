// [temp.res.general]/6, [dcl.type.elab]/3, [temp.inst]/3: a data member
// `class B<T>::In::Deep *d` is checked when W is instantiated: B<int> has no
// In (the same error as the `typename` form in X), and B<char>'s Deep is a
// union. Valid twin: member-elaborated-qualifier-nested-run.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 19 and 22 accept W<int> and W<char> silently (g++ 14 rejects W<int>: "no type named 'In' in 'struct B<int>'")
//@ diag: 21:43 error "no member named 'In' in 'B<int>'"
//@ diag: 24:8 note "in instantiation of template class 'W<int>' requested here"
//@ diag: 21:31 error "use of 'Deep' with tag type that does not match previous declaration"
//@ diag: 25:9 note "in instantiation of template class 'W<char>' requested here"
//@ diag: 20:48 note "previous use is here"
//@ diag: 22:46 error "no member named 'In' in 'B<int>'"
//@ diag: 26:8 note "in instantiation of template class 'X<int>' requested here"

template <class T> struct B { struct In { class Deep {}; }; };
template <> struct B<long> { struct In { class Deep { public: int y; }; }; };
template <class T> struct B<T *> { struct In { class Deep { public: int z; }; }; };
template <> struct B<int> { };
template <> struct B<char> { struct In { union Deep {}; }; };
template <class T> struct W { class B<T>::In::Deep *d; };
template <class T> struct X { typename B<T>::In::Deep *d; };
static_assert(sizeof(W<long>) == sizeof(void*));
W<int> wi;
W<char> wc;
X<int> xi;
