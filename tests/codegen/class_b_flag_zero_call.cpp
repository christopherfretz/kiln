// WP-813: `-fzero-call-used-regs=used-gpr` gives "zero-call-used-regs"="used-
// gpr" on every definition without its own attribute.
// The function attribute reaches every function kiln defines, its
// synthesized ones included (global initializers, TLS wrappers,
// thunks, lambdas), and the attributes that opt out of it win
// (spec/landed/codegen/WP-813.md).
//@ args: -fzero-call-used-regs=used-gpr
struct E { int v; E(int x) : v(x) {} ~E(); };
struct S { S(); ~S(); int n; };
struct B { virtual ~B(); virtual int f() { return 1; } };
struct D : B { int f() override { return 2; } };
B::~B() {}
thread_local S tl;
static S gs;
inline S is;
int g(int x) { if (x) throw E(x); return tl.n + gs.n + is.n; }
int h() { try { return g(1); } catch (E& e) { return e.v; } catch (...) { return 0; } }
int k(B* b) { auto l = [&] { return b->f(); }; return l(); }
int m() { static S loc; S a; return loc.n + a.n + g(0); }
template <class T> T tf(T t) { return t; }
int use() { return tf(1) + tf<char>(2); }
int g0();
__attribute__((no_instrument_function)) void ni() { g0(); }
__attribute__((no_split_stack)) void ns() {}
__attribute__((no_speculative_load_hardening)) void nslh() {}
__attribute__((speculative_load_hardening)) void slh() {}
__attribute__((patchable_function_entry(0))) void p0() {}
__attribute__((patchable_function_entry(1, 1))) void p1() {}
__attribute__((xray_always_instrument)) void xa() {}
__attribute__((xray_never_instrument)) void xn() {}
__attribute__((xray_log_args(1))) void xl(int) {}
__attribute__((zero_call_used_regs("skip"))) void zs() {}
__attribute__((zero_call_used_regs("used"))) void zu() {}
void loop(int n) { for (int i = 0; i < n; ++i) g0(); }
