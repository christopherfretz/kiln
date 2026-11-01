//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:22 error "invalid operands to binary expression ('R<Opaque>' and 'R<int>')"
//@ diag: 17:6 note "candidate template ignored: constraints not satisfied [with A = Opaque, B = int]"
//@ diag: 18:32 note "because 'x.base() == y.base()' would be invalid: invalid operands to binary expression ('Opaque' and 'int')"
//@ diag: 17:6 note "candidate template ignored: constraints not satisfied [with A = int, B = Opaque]"
//@ diag: 18:32 note "because 'x.base() == y.base()' would be invalid: invalid operands to binary expression ('int' and 'Opaque')"
//@ diag: 22:12 error "no matching function for call to 'f'"
//@ diag: 20:25 note "candidate template ignored: constraints not satisfied [with T = char]"
//@ diag: 20:41 note "because 'sizeof (a) > 1' (1 > 1) evaluated to false"
// [temp.constr.decl]/3.2: a trailing requires-clause that names a
// parameter is checked with the parameter's type substituted; when it is
// not satisfied the candidate is not viable. WP-121.
template <class T> struct R { T v; T base() const { return v; } };
struct Opaque {};
template <class A, class B>
bool operator==(const R<A> &x, const R<B> &y)
  requires requires { x.base() == y.base(); }
{ return true; }
template <class T> void f(T a) requires(sizeof(a) > 1) {}
bool b = R<Opaque>{} == R<int>{};
void g() { f('c'); }
