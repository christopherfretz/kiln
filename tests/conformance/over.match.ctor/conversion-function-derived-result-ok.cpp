// A direct-initialization of 'B' from a class with 'operator D() const'
// ('D' derived from 'B') and 'operator B&()': clang 22 and GCC 16 call the
// copy constructor through 'operator B&' at every -std level; by the text
// the move constructor, whose 'B&&' binds the 'D' prvalue directly
// ([dcl.init.ref]/5.3.2), is indistinguishable from it ([over.ics.rank]/3.3).
// The same holds for 'h(B&&)' beside 'h(const B&)'. Found running GCC's
// C++ testsuite.
//@ std: c++17
//@ expect: compile-ok
//@ xfail(kiln): WP-1625: a reference bound through a conversion function yielding a derived class (clang's and GCC's tie-break is not established)
struct B { B(); B(const B &); B(B &&); };
struct D : B {};
struct U { operator D() const; operator B &(); };
void h(B &&);
void h(const B &);
void f(U u) {
  B b(u);
  h(u);
}
