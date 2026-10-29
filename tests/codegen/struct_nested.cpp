// Nested classes and members of array type.
struct In { short s; char c; };
struct Out { In in; int i; };
struct Arr { int v[3]; };
Out eout(int); Arr earr(int);
int tout(Out o) { return o.in.s + o.in.c + o.i; }
int tarr(Arr a) { return a.v[0] + a.v[2]; }
Out pout(int x) { return eout(x); }
int nested(int x) { return tout(eout(x)) + tarr(earr(x)) + tout(pout(x)); }
