// [expr.ref]/7, [class.temporary]/2, [expr.const]: a member access on a
// class prvalue (a braced temporary, a call result, a conditional, a comma
// expression, a cast) materializes the temporary and is a constant
// expression; so are member calls on it, base members, anonymous-union
// members, bit-fields, array members and static members named through it.
// Regression for the WP-192 follow-up (libc++ span/at.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
struct P { int x, y; constexpr int sum() const { return x + y; } };
constexpr P mk(int a) { return P{a, a + 1}; }
static_assert(P{1, 2}.x == 1);
static_assert(mk(3).y == 4);
static_assert(mk(3).sum() == 7);
struct Q { P p; int arr[2]; };
static_assert(Q{{5, 6}, {7, 8}}.p.y == 6);
static_assert(Q{{5, 6}, {7, 8}}.arr[1] == 8);
constexpr int f() { return (true ? P{1,2} : P{3,4}).y; }
static_assert(f() == 2);
struct N { int v; constexpr ~N() {} };
constexpr int g() { return N{9}.v; }
static_assert(g() == 9);
constexpr const int &r = P{4,5}.y;
constexpr int h() { int&& rr = P{4,5}.y; return rr; }
static_assert(h() == 5);
static_assert(static_cast<P&&>(P{6,7}).y == 7);
struct Base { int b; };
struct D : Base { int d; };
static_assert(D{{1}, 2}.b == 1);
struct U { union { int i; float f; }; int k; };
static_assert(U{{3}, 4}.i == 3);
struct BF { int a : 3; int c : 5; };
static_assert(BF{1, 9}.c == 9);
constexpr D mkd() { return D{{5}, 6}; }
static_assert(mkd().b == 5);
static_assert(mkd().Base::b == 5);
struct S { int x; constexpr S(int v) : x(v) {} constexpr S(const S&o) : x(o.x+1) {} };
static_assert(S(1).x == 1);
constexpr int sz() { struct L { int a[3]; }; return L{{1,2,3}}.a[2]; }
static_assert(sz() == 3);
struct V { int x; constexpr int get() && { return x; } };
static_assert(V{7}.get() == 7);
template<class T> struct W { T t; };
static_assert(W<D>{D{{8},9}}.t.d == 9);
struct E { static constexpr int s = 11; int z; };
static_assert(E{1}.s == 11);
struct Pt { int x; };
struct Hold { Pt p; constexpr Pt get() const { return p; } };
constexpr Hold hh{{12}};
static_assert(hh.get().x == 12);
static_assert(((Pt{13})).x == 13);
constexpr int cm() { return ((void)Pt{1}, Pt{14}).x; }
static_assert(cm() == 14);
struct Ar { int a[2]; };
constexpr int ar() { return Ar{{1,15}}.a[1]; }
static_assert(ar() == 15);
static_assert(Ar{{1,16}}.a[1] == 16);
struct M { int v; constexpr M(int x):v(x){} constexpr ~M(){} };
static_assert(M(17).v == 17);
static_assert(__builtin_bit_cast(Pt, 18).x == 18);
