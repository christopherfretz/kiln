//@ expect: ast
// [dcl.init.ref]/4-5, [conv.qual]/3 (P0388R4): a reference to an array
// of unknown bound binds an array of known bound directly; the referent
// is adjusted by a NoOp cast to `T[]` (adding cv-qualifiers too)
int a[3];
int (&r)[] = a;
const int (&cr)[] = a;
int (&&rr)[] = static_cast<int(&&)[3]>(a);
void f(int (&)[]);
void g() { f(a); int (*p)[] = &a; (void)p; }
int (&r2)[] = r;
int m[3][2];
const int (&rm)[][2] = m;
