//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:9 error "cannot decompose private member 'p' of 'Pr'"
//@ diag: 12:16 note "implicitly declared private here"
//@ diag: 19:9 error "cannot decompose protected member 'x' of 'Pro'"
//@ diag: 13:29 note "declared protected here"
//@ requires: clang<22
// clang 22 emits error: cannot bind private member 'p' of 'Pr'; WP-355 residual
// [dcl.struct.bind]/5 (C++26 /7, P0969): every data member must be
// accessible at the declaration; clang names the member and notes where it
// became inaccessible. Friends and members may decompose.
class Pr { int p; public: int q; friend int fr(Pr); int mem(); };
struct Pro { protected: int x; };
struct PD : Pro { int g(PD d) { auto [x] = d; return x; } };
int Pr::mem() { auto [a, b] = *this; return a + b; }
int fr(Pr pr) { auto [a, b] = pr; return a + b; }
void f(Pr pr, Pro pro) {
  auto [pp, pq] = pr;
  auto [px] = pro;
}
