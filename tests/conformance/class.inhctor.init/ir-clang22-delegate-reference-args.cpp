// [class.inhctor.init], [expr.prim.lambda.closure]: an inheriting
// constructor and a lambda's static invoker forward a reference parameter
// as the pointer loaded from its slot with no `!nonnull` / `!align`
// metadata (clang `EmitDelegateCallArg` is a plain load), while a use of
// the reference in a body carries both at clang 22. Compared with clang
// 22's IR exactly.
//@ expect: ir
//@ requires: clang>=22
struct A { A(int &r, const double &d); };
struct B : A { using A::A; };
void inherit(int &x, double d) { B b(x, d); }
auto l = [](int &r, long long &q, int (&a)[3]) { return r + q + a[0]; };
long long (*fp)(int &, long long &, int (&)[3]) = l;
