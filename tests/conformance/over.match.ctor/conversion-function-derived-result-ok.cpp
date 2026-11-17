// A direct-initialization of 'B' from a class with 'operator D() const'
// ('D' derived from 'B') and 'operator B&()': clang 22 and GCC 16 call the
// copy constructor through 'operator B&' at every -std level; by the text
// the move constructor, whose 'B&&' binds the 'D' prvalue directly
// ([dcl.init.ref]/5.3.2, [over.match.ref]/1.1 sets 'operator B&' aside),
// is indistinguishable from it ([over.ics.rank]/3.3). The same holds for
// 'h(B&&)' beside 'h(const B&)'. Both compilers weigh the lvalue-yielding
// function for the 'B&&' too and find it not viable; kiln follows both
// (a recorded deviation, docs/status/deviations.md). Found running GCC's
// C++ testsuite.
//@ std: c++11
//@ expect: compile-ok
struct B { B(); B(const B &); B(B &&); };
struct D : B {};
struct E : B {};
struct U1 { operator D() const; operator B &(); };
struct U2 { operator D(); operator B &(); };
struct U3 { operator D() const; operator B &() const; };
struct U4 { operator D() const; operator D &(); };
struct U5 { operator E() const; operator D &(); };
struct U6 { operator D() const; operator const B &(); };
void h(B &&);
void h(const B &);
template <class U> void f(U u) {
  B b(u);
  B c = u;
  h(u);
}
template void f(U1);
template void f(U2);
template void f(U3);
template void f(U4);
template void f(U5);
template void f(U6);
