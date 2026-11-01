// Zero-initialization of objects holding a pointer to data member, whose
// null value is -1 under the Itanium ABI ([dcl.init.general]/6): clang's
// `isZeroInitializable`, `EmitNullConstant` and `EmitNullInitialization`.
// Value-initialized locals, temporaries and `new`-expressions copy a
// private null constant instead of `memset`ing 0; globals, statics and
// constants store -1; base-class subobjects use the base null constant.
struct T { int x; int T::*p; };
struct U { U(); int T::*q; };
struct B { int T::*p; int y; };
struct D : B { D() : B() {} int z; };
struct N { int a; T t[2]; };
struct V { virtual void f(); int T::*p; };
struct W { W() = default; int T::*p; char c[40]; };
T g{};
T ga[3]{};
N gn{};
W gw{};
void use(void*);
void f1() { T t{}; use(&t); }
void f2() { T t = T(); use(&t); }
T* f3() { return new T(); }
void f4() { T a[3]{}; use(a); }
void f5() { D d; use(&d); }
void f6() { N n{}; use(&n); }
void f7() { V v{}; use(&v); }
V* f8() { return new V(); }
W* f9() { return new W(); }
void f10() { W w{}; use(&w); }
void f11() { W w = W(); use(&w); }
T* f12(int n) { return new T[n](); }
V* f13(int n) { return new V[n](); }
void f14() { static T s{}; use(&s); }
int T::*f15() { return T().p; }
// Globals: constant and dynamic initialization, no initializer.
T g1{};
T g2 = {1};
T g3 = {1, nullptr};
T g4 = {1, &T::x};
constexpr T g5{};
const T* q = &g5;
int T::*g6{};
int T::*g7;
T g8;
// Unions, virtual bases, member function pointers, bases, arrays.
union U1 { int T::*p; int i; };
union U2 { int i; int T::*p; };
struct VB { int T::*p; };
struct VD : virtual VB { VD() : VB() {} int k; };
struct F { void (T::*mf)(); int T::*dp; };
struct E : T { int e; };
struct Big { int T::*p[8]; char c[40]; };
U1 gu1{};
U2 gu2{};
U1 gu1b;
F gf{};
E ge{};
Big gb{};
int T::*garr[4]{};
int T::*garr2[4]{&T::x};
void h1() { U1 u{}; use(&u); U2 v{}; use(&v); }
void h2() { VD d; use(&d); }
void h3() { F f{}; use(&f); E e{}; use(&e); E e2 = E(); use(&e2); }
void h4() { Big b{}; use(&b); Big b2 = Big(); use(&b2); }
void h5(int n) { use(new int T::*[n]()); use(new int T::*[n][2]()); use(new T[3]()); use(new T[n]{{1, &T::x}}); }
void h6() { int T::*a[4]{}; use(a); int T::*q{}; use(&q); }
void h7(T* t) { *t = T(); *t = {}; }
struct H { H() = default; T t; int T::*m; };
void h8() { H h{}; use(&h); H h2 = H(); use(&h2); use(new H()); use(new H[2]()); }
struct I { T t; int y; I() : t() {} };
void h9() { I i; use(&i); }
typedef void (T::*MF)();
void h10() { use(new (int T::*)()); use(new (int T::*)(&T::x)); use(new MF()); use(new (int T::*)); }
