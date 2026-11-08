// WP-813: class B attributes beside cleanups, static-local guards and
// `throw`: the declarations' attributes, and none on the runtime helpers.
// spec/landed/codegen/WP-813.md.
struct D { ~D(); };
D d;
void f() { static D s; throw 1; }
__attribute__((no_speculative_load_hardening)) void nslh();
__attribute__((no_split_stack)) void nss();
void u() { nslh(); nss(); }
