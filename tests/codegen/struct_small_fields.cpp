// Small classes of 2, 3 and 4 bytes.
struct S2 { char a, b; };
struct S3 { char a, b, c; };
struct SS { short a; char b; };
S2 e2(int); S3 e3(int); SS ess(int);
int t2(S2 s) { return s.a + s.b; }
int t3(S3 s) { return s.a + s.c; }
int tss(SS s) { return s.a * s.b; }
S3 p3(int x) { return e3(x); }
int small(int c) { return t2(e2(c)) + t3(e3(c)) + tss(ess(c)) + t3(p3(c)); }
