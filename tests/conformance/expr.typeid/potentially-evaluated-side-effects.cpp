// [expr.typeid]/3: a polymorphic glvalue operand is evaluated, so side effects in
// it warn (-Wpotentially-evaluated-expression); not in instantiations (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 19:16 warning "expression with side effects will be evaluated despite being used as an operand to 'typeid'"
//@ diag: 22:16 warning "expression with side effects will be evaluated despite being used as an operand to 'typeid'"
//@ diag: 23:17 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 25:16 warning "expression with side effects will be evaluated despite being used as an operand to 'typeid'"
//@ diag: 26:16 warning "expression with side effects will be evaluated despite being used as an operand to 'typeid'"
//@ diag: 28:48 warning "expression with side effects will be evaluated despite being used as an operand to 'typeid'"
namespace std { class type_info; }
struct P { virtual ~P(); };
struct N { int x; };
P &gp();
N &gn();
P *pp;
int i;
void f() {
  (void)typeid(gp());
  (void)typeid(gn());
  (void)typeid(*pp);
  (void)typeid(*pp++);
  (void)typeid(i++);
  (void)typeid(P(gp()));
  (void)typeid(pp[i++]);
  (void)typeid((i = 1, *pp));
}
template <class T> void g(T &t) { (void)typeid(gp()); (void)typeid(t); }
template void g<P>(P &);
