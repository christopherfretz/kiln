// Assignment operator calls evaluate the right operand first
// ([expr.ass]/1: the right operand is sequenced before the left;
// [over.match.oper]/2: an overloaded assignment follows the built-in
// rule), as clang's `EvaluationOrder::ForceRightToLeft` does: a trivial
// copy/move assignment emits the source lvalue, then the object, then the
// memcpy (empty classes copy nothing); a user-provided `operator=` or
// compound assignment emits its argument, then the object, then names the
// callee. An ignored call returning an empty class (ABI `Ignore`) still
// gets its unused temporary (clang `GetUndefRValue`), made after the
// arguments.
struct S { int a, b; };
struct E {};
struct Big { int x[10]; };
S &l();
S r();
S &lr();
E &el();
E er();
Big &bl();
Big br();
void f1() { l() = r(); }
void f2() { l() = lr(); }
void f3() { el() = er(); }
void f4() { bl() = br(); }
S g;
void f5(S s) { g = s; }
void f6() { (l() = r()).a = 3; }
int f7() { return (l() = lr()).b; }
struct N { int a; N &operator=(const N &); N &operator+=(int); N &operator<<=(N); };
N &nl();
N nr();
int ir();
void g1() { nl() = nr(); }
void g2() { nl() += ir(); }
void g3() { nl() <<= nr(); }
void g4() { nl().operator=(nr()); }
void take(const E &);
E er2(E, E);
struct M { E m(E); Big mb(Big); };
void h1() { E x = er(); }
void h2() { er2(E{}, E{}); br(); M m; m.m(E{}); m.mb(Big{}); }
void h3() { const E &r = er(); }
void h4() { take(er()); }
