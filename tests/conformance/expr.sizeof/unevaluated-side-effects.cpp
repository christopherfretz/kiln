// [expr.context]/1: the operands of sizeof, alignof, decltype, noexcept
// and a non-polymorphic typeid are unevaluated, so their side effects
// do not happen (clang's -Wunevaluated-expression, in -Wmost and on by
// default; [expr.sizeof]/1, [dcl.type.decltype]/1, [expr.unary.noexcept]/1,
// [expr.typeid]/4).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 37:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 38:17 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 39:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 40:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 41:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 42:27 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 43:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 46:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 47:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 48:22 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 50:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 51:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 52:17 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 53:14 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 54:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 55:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 56:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 57:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 58:18 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 60:18 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 61:17 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 62:18 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 71:17 warning "expression with side effects has no effect in an unevaluated context"
namespace std { class type_info; }
int g(int);
int h() noexcept;
struct P { int m; int f(int); };
#define INC(x) x++
void f(int i, int *p, P *s, P &r) {
  (void)sizeof(i++);
  (void)sizeof i++;
  (void)sizeof(i = 1);
  (void)sizeof(i += 2);
  (void)sizeof(--i);
  (void)alignof(decltype(i++));
  (void)sizeof(g(i++));
  (void)sizeof(g(i));               // a call alone has no side effects
  (void)sizeof(i + 1);
  (void)sizeof(new int);
  (void)sizeof(s->m++);
  (void)sizeof(sizeof(i++));        // once, for the inner operand
  (void)sizeof(INC(i));             // written in a macro: none
  decltype(p[i++]) a = i;
  decltype(i ? i++ : 0) b = 0;
  decltype(s->m = 1) c = i;
  decltype(r.f(i++)) d = 0;
  decltype(i++, i) e = i;
  decltype(static_cast<long>(i++)) k = 0;
  decltype(-(i++)) m = 0;
  decltype(*(p = 0)) n = i;
  (void)noexcept(g(i++));
  (void)noexcept(h());
  (void)noexcept(throw 1);
  (void)typeid(i++);
  (void)typeid(p = 0);
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)k; (void)m; (void)n;
}
// In a template: a dependent operand is never reported; a non-dependent
// one is reported in the pattern, not again at instantiation.
template<class T> struct TT {
  static void t(T x) {
    (void)sizeof(x++);
    int n = 0;
    (void)sizeof(n++);
    (void)sizeof(sizeof(T) + n++);
  }
};
void u() { TT<int>::t(1); TT<long>::t(1); }
