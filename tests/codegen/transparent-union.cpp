// GNU `transparent_union` (WP-809): in C++ a kept class-head attribute
// makes no conversions, but a by-value parameter of the union is passed
// as its first field (clang `useFirstFieldIfTransparentUnion`); returns
// and unions whose attribute was dropped are passed as plain unions.
union __attribute__((transparent_union)) U { int *i; long l; };
union [[gnu::transparent_union]] G { long l; int *i; };
union P { int *i; long l; };
typedef union { int *i; long l; } TY __attribute__((transparent_union));
union Z { int *i; long l; } __attribute__((transparent_union));
union __attribute__((transparent_union)) J { int a; char b; };
union __attribute__((transparent_union)) D { double d; long l; };
union __attribute__((transparent_union)) S { struct { int a, b; } s; long l; };

long fu(U u) { return u.l; }
long fg(G g) { return g.l; }
long fp(P p) { return p.l; }
long fty(TY t) { return t.l; }
long fz(Z z) { return z.l; }
int fj(J j) { return j.a; }
long fd(D d) { return d.l; }
long fs(S s) { return s.l; }
U ret(int *p) { U u; u.i = p; return u; }
long call(int *p) { U u; u.i = p; G g; g.l = 1; return fu(u) + fg(g) + fu(ret(p)); }
