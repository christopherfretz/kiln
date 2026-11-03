// [class.friend]/3, [dcl.fct]/6 (CWG1417): `friend T;` naming a non-class
// type is ignored, so an unqualified function typedef is accepted, but a
// function typedef with a cv-qualifier-seq or ref-qualifier is not the
// type of a non-static member function: clang's
// `err_invalid_qualified_function_type` "non-member function of type" form
// at the start of the declaration. A dependent `friend T;` that
// substitutes to such a type is not diagnosed; a non-dependent one in a
// template is reported once, in the pattern (clang 19 and 22).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "non-member function of type 'G' (aka 'int (int) const') cannot have 'const' qualifier"
//@ diag: 25:3 error "non-member function of type 'H' (aka 'int (int) &') cannot have '&' qualifier"
//@ diag: 26:3 error "non-member function of type 'A' (aka 'void () volatile') cannot have 'volatile' qualifier"
//@ diag: 27:3 error "non-member function of type 'N::R' (aka 'void () const &&') cannot have 'const &&' qualifier"
//@ diag: 33:31 error "non-member function of type 'G' (aka 'int (int) const') cannot have 'const' qualifier"
typedef void F();
typedef int G(int) const;
typedef int H(int) &;
using A = void() volatile;
namespace N { typedef void R() const &&; }

struct S {
  friend F;
  friend G;
  friend H;
  friend A;
  friend N::R;
};
template <class T> struct U { friend T; };
U<G> u;
template <class T> struct V { typedef T X; friend X; };
V<H> v;
template <class T> struct W { friend G; };
W<int> w;
