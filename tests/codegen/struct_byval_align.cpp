// A 24-byte class of chars: x86-64 byval align 8 raises the alignment of
// the argument temporary (getOrEnforceKnownAlignment).
struct C24 { char c[24]; };
C24 ext(int);
int take(C24 s) { return s.c[0] + s.c[23]; }
int use(int x) { return take(ext(x)); }
