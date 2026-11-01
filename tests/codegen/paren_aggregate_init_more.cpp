// Parenthesized aggregate initialization (P0960) beyond
// paren_aggregate_init.cpp: trailing default member initializers,
// mem-initializers of members and bases, destructible temporaries in the
// list, dynamic and static-local initialization, references, empty bases.
struct Agg { int a; int b = 7; double c; };
struct Nest { Agg g; int z; };
struct Bs : Agg { int q; };
void use(void*);
void f1(int x) { Agg a(3); use(&a); Agg b(x, x); use(&b); Agg c(x, 2, 3.5); use(&c); }
void f2(int x) { int a[3](1, 2); use(a); int b[](x, x, x); use(b); int c[4](x); use(c); }
void f3(int x) { Nest n(Agg(x), 4); use(&n); Bs s(Agg(x), 9); use(&s); }
Agg* f4(int x) { return new Agg(x, 1); }
int* f5(int x) { return new int[3](x, 2); }
Agg g1(5);
int g2[3](1, 2);
Agg g3(1, 2, 3.0);
Agg f6(int x) { return Agg(x); }
void f7(int x) { use(new Agg[2](Agg(x), Agg(1, 2))); }
struct R { int& r; int v; };
void f8(int x) { R r(x); use(&r); R r2(x, 3); use(&r2); }
struct D { ~D(); int k; };
struct HD { D d; int m; };
void f9(int x) { HD h(D(), x); use(&h); }

union U { int i; float f; };
void take(Agg);
int f();
struct X { Agg a; int arr[2]; X(int v) : a(v, 2), arr(v) {} };
struct Y : Agg { Y(int v) : Agg(v, v) {} };
void q1(int x) { X q(x); use(&q); Y y(x); use(&y); }
void q2(int x) { take(Agg(x, 1)); U u(x); use(&u); }
int* q3(int n) { return new int[n](1, 2); }
Agg* q4(int n) { return new Agg[n](Agg(1), Agg(2)); }
void q5(int x) { int a[2][2]({x}, {1, 2}); use(a); }
Agg gd(f(), f());
HD ghd(D(), 3);
void q6() { static Agg s(f()); use(&s); }
constexpr Agg ce(1, 2, 3.0);
const Agg* q7() { return &ce; }
void q8(int x) { HD h(D{x}, f()); use(&h); }
struct E {};
struct WithE : E { int v; };
void q9(int x) { WithE w(E(), x); use(&w); }
auto q10(int x) { return Agg(x, x, x); }
void q11(int x) { auto&& r = Agg(x); use(&r); }
