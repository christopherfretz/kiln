//@ expect: ast
//@ oracle: clang
//@ ast-filter: f
// [class.temporary]/6: the contexts that extend a temporary's lifetime to
// that of the reference bound to it: class member access, array subscript
// of an array temporary, parentheses, the right operand of a comma, both
// arms of a conditional, and the reference members of an aggregate
// (both an aggregate variable and an aggregate temporary that is itself
// extended). Temporaries of a discarded expression or a function argument
// are only bound and destroyed at the end of the full-expression
// (ExprWithCleanups). Matches clang 19.
namespace q {
struct S { int m; int arr[2]; ~S(); };
struct A { const S &r; int n; };
S mk();
void use(const S &);
void f(bool b) {
  const int &r1 = mk().m;
  const int &r2 = mk().arr[1];
  const S &r3 = (mk());
  const S &r4 = (use(mk()), mk());
  const S &r5 = b ? mk() : mk();
  A a1{mk(), 1};
  const A &r7 = A{mk(), 2};
  use(mk());
  mk();
}
}
