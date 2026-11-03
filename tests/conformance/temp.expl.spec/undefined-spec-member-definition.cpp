// [temp.expl.spec]/17, [temp.inst]/2: a member of a member template
// specialization must be declared by it; through a declared but
// undefined explicit specialization the qualifier is incomplete.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:46 error "out-of-line definition of 'g' does not match any declaration in 'A2<long>::B<int>'"
//@ diag: 16:56 note "B defined here"
//@ diag: 20:28 error "incomplete type 'A2<char>' named in nested name specifier"
//@ diag: 19:19 note "forward declaration of 'A2<char>'"
//@ diag: 23:12 error "incomplete type 'A3<char>' named in nested name specifier"
//@ diag: 22:19 note "forward declaration of 'A3<char>'"
//@ diag: 24:17 error "incomplete type 'A3<char>' named in nested name specifier"
//@ diag: 22:19 note "forward declaration of 'A3<char>'"
//@ diag: 25:28 error "incomplete type 'A3<char>' named in nested name specifier"
//@ diag: 22:19 note "forward declaration of 'A3<char>'"
template<class T> struct A2 { template<class U> struct B { void f(); }; };
template<> template<> void A2<long>::B<int>::f() {}
template<> template<> void A2<long>::B<int>::g() {}
template<> struct A2<char>;
template<> template<> void A2<char>::B<int>::f() {}
template<class T> struct A3 { template<class U> struct B { void f(); }; void k(); };
template<> struct A3<char>;
void h() { A3<char>::k(); }
template<> void A3<char>::k() {}
template<> template<> void A3<char>::B<int>::f() {}
