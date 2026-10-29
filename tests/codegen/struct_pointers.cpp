// Classes of pointers and pointer/integer mixes.
struct PP { int* p; int* q; };
struct PI { const char* s; int n; };
struct P1 { void* p; };
PP epp(int*); PI epi(int); P1 ep1(int);
int tpp(PP v) { return *v.p + *v.q; }
char tpi(PI v) { return v.s[v.n]; }
void* tp1(P1 v) { return v.p; }
P1 pp1(int x) { return ep1(x); }
int ptrs(int* a) { return tpp(epp(a)) + tpi(epi(1)) + (tp1(ep1(2)) != tp1(pp1(3))); }
