// [dcl.ambig.res]/1: after a declarator-id, `(` starts a parameter list
// only if its contents can be a parameter-declaration-clause. A
// parenthesized functional cast `((T()))` cannot, so it is a direct
// initializer whose expression is the parenthesized temporary; inside
// it, the ambiguous type-id `(T())` is a cast only if a cast-expression
// follows the `)` ([dcl.ambig.res]/2, [expr.cast]; clang's
// ParseCXXAmbiguousParenExpression). `T f(T(h))`, `T f(T(*p))`,
// `T f(T())` and `T f(T q)` are function declarations. From libc++
// support/count_new.h:
// `static MemCounter counter((MemCounter::MemCounterCtorArg_()));`.
struct M { class Arg {}; explicit M(Arg); };
struct C { C(); C(int); };
int g;
static M amb_m((M::Arg()));
C amb_t((C()));
C amb_tt(((C())));
C amb_a((g));
C amb_ta((C(g)));
int amb_i((int()));
C amb_f1(C(h));
C amb_f2(C(*p));
C amb_f3(C());
C amb_f4(C q);
void amb_body() {
  M m((M::Arg()));
  C c((C(g)));
  int y = (int()) / 2;
  (void)(C());
}
