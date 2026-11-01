// [class.mfct.non.static]/2, [expr.prim.id]/3: `B::m` inside a member of a
// class template with a dependent base is checked at instantiation; when
// `B` turns out not to be a base, the member cannot be named that way.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:23 error "'B::h' is not a member of class 'D<O>'"
//@ diag: 17:31 note "in instantiation of member function 'D<O>::g' requested here"
//@ diag: 15:23 error "'B::x' is not a member of class 'D<O>'"
//@ diag: 17:39 note "in instantiation of member function 'D<O>::y' requested here"
struct B { int x; int h(); };
struct O {};
template <class T> struct D : T {
  int g() { return B::h(); }
  int y() { return B::x; }
};
int main() { D<O> d; return d.g() + d.y(); }
