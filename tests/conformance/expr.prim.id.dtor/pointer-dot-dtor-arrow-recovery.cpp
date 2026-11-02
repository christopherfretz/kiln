// [expr.prim.id.dtor]/2, [expr.ref]/2: `p.~T()` with `p` a pointer to `T`
// names no destructor of the pointer type; after "did you mean to use
// '->'" the access goes on as `p->~T()` (clang `BuildPseudoDestructorExpr`),
// so the call is a `void` expression for a class and a scalar alike, and a
// qualified name's scope type is then checked against `T`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:12 error "member reference type 'S *' is a pointer; did you mean to use '->'?"
//@ diag: 28:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
//@ diag: 29:12 error "member reference type 'I *' (aka 'int *') is a pointer; did you mean to use '->'?"
//@ diag: 29:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
//@ diag: 30:12 error "member reference type 'S *' is a pointer; did you mean to use '->'?"
//@ diag: 30:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
//@ diag: 31:4 error "member reference type 'S *' is a pointer; did you mean to use '->'?"
//@ diag: 31:5 error "the type of object expression ('S') does not match the type being destroyed ('U') in pseudo-destructor expression"
//@ diag: 32:4 error "member reference type 'I *' (aka 'int *') is a pointer; did you mean to use '->'?"
//@ diag: 33:4 error "member reference type 'U *' is a pointer; did you mean to use '->'?"
//@ diag: 34:43 error "variable has incomplete type 'void'"
//@ diag: 34:48 error "member reference type 'S *' is a pointer; did you mean to use '->'?"
//@ diag: 37:36 error "member reference type 'U *' is a pointer; did you mean to use '->'?"
//@ diag: 38:15 note "in instantiation of function template specialization 't<U>' requested here"
struct S { ~S(); };
struct U { ~U(); };
using I = int;
namespace N { using J = int; }
using V = decltype(static_cast<S *>(nullptr)->~S());
void q(S *p, I *i, U *u) {
  int a = p.~S();
  int b = i.~I();
  int c = p.S::~S();
  p.U::~S();
  i.N::J::~J();
  u.~U();
  static_assert(__is_same(V, void)); auto v = p.~S();
  p->~S();
}
template <class T> void t(T *p) { p.~T(); }
template void t<U>(U *);
