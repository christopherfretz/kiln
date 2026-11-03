// WP-416: an inheriting constructor and a lambda's static invoker forward
// a reference parameter as a plain load of its slot (clang
// `EmitDelegateCallArg`; no `!nonnull` / `!align` at 22).
struct A { A(int &r, const double &d); };
struct B : A { using A::A; };
void inherit(int &x, double d) { B b(x, d); }
auto l = [](int &r, long long &q, int (&a)[3]) { return r + q + a[0]; };
long long (*fp)(int &, long long &, int (&)[3]) = l;
