// [class.copy.assign]/12.2: an implicit copy/move assignment assigns an
// array member of class type element by element (clang
// `buildSingleCopyAssignRecursively`: one `size_t __iD` loop per dimension).
struct E { E& operator=(const E&); E& operator=(E&&); int v; };
struct S { E a[2]; int n; E b[2][3]; };
void copy(S& x, const S& y) { x = y; }
void move(S& x, S& y) { x = static_cast<S&&>(y); }
struct T : S { E c[4]; };
void tcopy(T& x, const T& y) { x = y; }
// The right operand of an assignment operator is evaluated first
// ([expr.ass]/1).
S& get(int);
void order() { get(0) = get(1); }
void order_move() { get(0) = static_cast<S&&>(get(1)); }
struct P { int i; };
P& getp(int);
void order_trivial() { getp(0) = getp(1); }
struct W { W& operator+=(int); virtual W& operator=(const W&); };
W& getw(int);
int geti();
void order_compound() { getw(0) += geti(); }
void order_virtual(W* w) { *w = getw(1); }
