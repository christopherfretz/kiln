// Accesses for the null / alignment / object-size checks (clang
// EmitTypeCheck; WP-871).
void *operator new(decltype(sizeof 0), void *) noexcept;
namespace n1 {
struct S { int a; char c; int arr[4]; void m(); };
int ld(int *p) { return *p; }
void st(int *p, int v) { *p = v; }
int mem(S *s) { return s->a; }
int mem2(S &s) { return s.a; }
int ref(int &r) { return r; }
int sub(int *p, int i) { return p[i]; }
int arr(S *s, int i) { return s->arr[i]; }
void call(S *s) { s->m(); }
int &bind(int *p) { return *p; }
int loc() { int x = 1; int *p = &x; return *p; }
} // namespace n1
namespace n2 {
struct B { int b; virtual ~B(); };
struct S { int a; char c; int arr[4]; void m(); int g() { return a + this->a; } S &operator=(const S&) = default; };
struct D : B { int d; D(); };
void ca(int *p, int v) { *p += v; }
void aa(S *x, S *y) { *x = *y; }
S al(S *x) { return *x; }
int &rv(int *p) { int &r = *p; return r; }
void take(int &);
void args(int *p) { take(*p); }
void S::m() { a = 1; }
B *up(D *d) { return d; }
D *down(B *b) { return static_cast<D*>(b); }
D &downr(B &b) { return static_cast<D&>(b); }
void del(S *s) { delete s; }
S *nw() { return new S; }
D::D() : d(1) {}
void ctor(D *p) { new (p) D; }
int g(S *s) { return s->g(); }
void inc(int *p) { ++*p; (*p)++; }
int lam(int *p) { return [p] { return *p; }() ^ [](int *q) { return *q; }(p); }
} // namespace n2
namespace n3 {
int mv(int b) { return b; }
int mv2(int b) { b += 1; return b; }
struct C { void operator()(int); int &operator[](int); C &operator++(); };
int op(C *c, int i) { (*c)(i); ++*c; return (*c)[i]; }
} // namespace n3
