// [expr.throw]/1, [class.temporary]/4: the temporaries of a
// throw-expression's full-expression are destroyed when control leaves
// it, which it never does normally; that implicit destruction is not
// dead code (-Wunreachable-code; clang's CFG has no element for the
// `ExprWithCleanups`). Code after the throw still is.
//@ args: -Wunreachable-code
//@ strict-diags
//@ diag: 17:26 warning "code will never be executed"
struct E { E(const char*); ~E(); };
void* g() { throw E(""); }
struct F { F(const char*); virtual ~F(); };
void* h() { throw F(""); }
struct S { S(const char*); };
struct G { G(const S&); ~G(); };
void* i() { throw G(""); }
int k(E);
void j() { throw E("a"); k(E("b")); }
