// _Complex values across calls and memory: direct and indirect returns
// for every element type (AAPCS HFA / x86-64 SSE classes), discarded
// results, conditionals, members and compound assignment (WP-211).
using C = _Complex double;
using CF = _Complex float;
using CL = _Complex long double;
using CI = _Complex int;
C f(C);
CF ff(CF);
CL fl(CL);
CI fi(CI);
double use(C a, CF b, CL c, CI d) { C x = f(a); CF y = ff(b); CL z = fl(c); CI w = fi(d); return __real__ x + __imag__ y + __real__ z + __imag__ w; }
void ign(C a) { f(a); a + a; }
C cond(bool b, C x, double d) { return b ? x : d; }
struct S { C c; int i; };
double mem(S s) { S t = {{1,2}, 3}; return __real__ s.c + __imag__ t.c; }
void cassign(C& a, C b, double d) { a = b; a *= b; a /= d; a -= 1; }
