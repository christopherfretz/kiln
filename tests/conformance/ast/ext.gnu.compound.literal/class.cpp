// Class compound literals (WP-191): a class prvalue of the written type,
// bound to a temporary when the destructor is non-trivial; a class with
// constructors is list-initialized as by an explicit cast (a
// CXXTemporaryObjectExpr). Instantiation: see class-run.cpp.
struct P { int a, b; };
struct D { int a; ~D(); };
struct Q { Q(int); int v; };
struct E { E(int, int); ~E(); int v; };
void take(D);
int f() { return (P){1,2}.b; }
D g() { D d = (D){3}; take((D){4}); const D &r = (D){5}; return (D){6}; }
int h() { return (D){1}.a + (Q){7}.v + (E){1, 2}.v; }
constexpr int k() { return (P){8,9}.b; }
static_assert(k() == 9);
