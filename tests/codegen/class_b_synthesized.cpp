// WP-813: the class B attributes and the functions kiln synthesizes
// (global initializers, the TLS wrapper and init, vtable thunks) and a
// lambda's call operator: the synthesized ones get no attribute-derived
// effects, and an attributed function keeps its own.
// spec/landed/codegen/WP-813.md.
int g();
int x = g();
thread_local int t = g();
int &rt() { return t; }
struct A { virtual void f(); }; struct B { virtual void h(); };
struct C : A, B { void h() override; };
void C::h() {}
void use() { auto l = []{ return 1; }; l(); g(); }
__attribute__((no_instrument_function)) void ni() { g(); }
__attribute__((no_split_stack)) void ns() {}
__attribute__((no_speculative_load_hardening)) void nslh() {}
__attribute__((patchable_function_entry(0))) void p0() {}
__attribute__((xray_always_instrument)) void xa() {}
__attribute__((xray_never_instrument)) void xn() {}
__attribute__((xray_log_args(1))) void xl(int) {}
__attribute__((zero_call_used_regs("skip"))) void zs() {}
