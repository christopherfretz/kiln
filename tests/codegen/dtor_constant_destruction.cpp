// [expr.const]/7, [basic.start.term]/1: a variable whose destructor is
// non-trivial and constexpr gets no destructor call (and no __cxa_atexit
// registration) when it has constant destruction -- clang
// `Sema::FinalizeVarWithDestructor` / `VarDecl::needsDestruction`
// (spec/codegen.md "Constant destruction"). Only a const variable's
// object is accessible to its destructor (holding its constant value, or
// indeterminate), never its mutable members; a constant-initialized
// variable is destroyed in a manifestly constant-evaluated context.
extern "C" int puts(const char*);
struct S { constexpr ~S() {} };
struct M { S s; int i; };                      // implicit constexpr dtor
struct U { ~U() {} };                          // not constexpr: kept
struct A { int x; constexpr ~A() { x = 5; } };
struct B { int x; constexpr ~B() { if (x) {} } };
struct C { constexpr ~C() { if (!__builtin_is_constant_evaluated()) puts("x"); } };
struct H { constexpr ~H() { (void)this; int y = 0; y++; } };
struct I { int x; constexpr ~I() { int *p = &x; (void)p; } };
struct Mu { mutable int m; constexpr ~Mu() { m = 1; } };
struct Mr { mutable int m; constexpr ~Mr() { if (m) {} } };
struct K { int k; constexpr K() : k(0) {} constexpr ~K() { k = 2; } };
struct P { int *p; constexpr ~P() { delete p; } };
struct Q { int x; constexpr ~Q() { if (x == 1) throw 1; } };
void use(const void*);

void elided(int n) {
  S s; M m; S arr[2]; H h; I i{0};
  const A ca{1};          // writes its own const object
  const A cn{n};          // indeterminate value, only written
  const B cb{1};          // reads its constant value
  const K ck;
  const P cp{nullptr};
  const Q q2{2};
  constexpr B xb{1};
  static S st;
  use(&s); use(&m); use(arr); use(&h); use(&i);
}

void kept(int n, int *q) {
  U u; A a; B b; B b1{1}; B bn{n}; C c;
  Mu mu{0}; K k;
  const Mu cmu{0}; const Mr cmr{0};
  const Q q1{1};
  S{};
  const S &r = S{};
  static B sb;
  use(&u); use(&r);
}

S gs;                       // no __cxa_atexit
const B gcb{1};
constexpr B gcb2{1};
C gc;                       // constant-initialized: constant context
B gb;                       // __cxa_atexit
B gb1{1};
K gk;
constinit K gk2;
static A sa{1};
const Q gq{1};
