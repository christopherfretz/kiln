// [dcl.attr.assume]/1 (WP-160): the assumption is a conditional-expression
// contextually converted to bool, on a null statement only. One with
// possible side effects (clang `HasSideEffects(IncludePossibleEffects)`:
// a call to a function neither const nor pure, a volatile read, `new`, a
// non-trivial constructor, a dynamic_cast to a reference, a capture
// initializer with effects) is ignored with -Wassume; in a template that
// is checked on instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 43:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 45:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 46:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 47:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 48:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 50:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 51:25 error "use of this expression in an 'assume' attribute requires parentheses"
//@ diag: 52:18 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 53:26 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 54:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 59:5 error "'assume' attribute only applies to empty statements"
//@ diag: 60:5 error "'assume' attribute takes one argument"
//@ diag: 61:14 error "use of this expression in an 'assume' attribute requires parentheses"
//@ diag: 62:17 error "use of this expression in an 'assume' attribute requires parentheses"
//@ diag: 63:12 error "value of type 'S' is not contextually convertible to 'bool'"
//@ diag: 64:5 error "'assume' attribute only applies to empty statements"
//@ diag: 65:5 error "'assume' attribute cannot be applied to a declaration"
//@ diag: 66:12 error "expected expression"
//@ diag: 68:3 error "'assume' attribute cannot be applied to a declaration"
//@ diag: 72:5 error "'assume' attribute only applies to empty statements"
//@ diag: 71:12 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 74:9 note "in instantiation of function template specialization 't<int>' requested here"
int g(int);
[[gnu::const]] int cg(int);
[[gnu::pure]] int pg(int);
volatile int vv;
struct NT { NT(); bool ok() const; };
struct B { virtual ~B(); };
struct D : B {};
int f(int x, B& b) {
  [[assume(x > 0)]];
  [[assume(x++ > 0)]];
  [[assume(g(x))]];
  [[assume(cg(x) && pg(x))]];
  [[assume(vv)]];
  [[assume(new int)]];
  [[assume(NT().ok())]];
  [[assume(&dynamic_cast<D&>(b) != nullptr)]];
  [[assume(dynamic_cast<D*>(&b))]];
  [[assume([y = x++] { return y; }())]];
  [[assume(x), assume(x = 1 > 0 ? 1 : 2)]];
  [[assume(sizeof(x++) == 4)]];
  [[assume(x)]] [[assume(g(x))]];
  [[assume((x = 2))]];
  return x;
}
struct S { int v; };
void h(int x, S s) {
  [[assume(x > 0)]] return;
  [[assume]];
  [[assume(x = 1)]];
  [[assume(x = 1, x)]];
  [[assume(s)]];
  [[assume(x)]] { }
  [[assume(x)]] int y;
  [[assume()]];
}
[[assume(true)]] int z;
template <class T> T t(T x) {
  [[assume(x > 0)]];
  [[assume(x++)]];
  [[assume(x)]] return x;
}
int u = t(1);
