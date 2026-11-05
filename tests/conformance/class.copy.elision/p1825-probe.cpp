// [class.copy.elision]/3 (C++20, P1825R0): implicit move from local
// objects and rvalue-reference parameters in return statements, including
// through a converting constructor taking an rvalue. WP-646 feature probe.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout: m m c m .
extern "C" int printf(const char*, ...);
struct M {
  M() {}
  M(const M&) { printf("c "); }
  M(M&&) { printf("m "); }
};
struct Sink { Sink(M&&) { printf("m "); } Sink(const M&) { printf("c "); } };
M from_rref(M&& m) { return m; }
Sink convert() { M m; return m; }
M from_lref(M& m) { return m; }
M local(bool b) { M x; M y; return b ? static_cast<M&&>(x) : static_cast<M&&>(y); }
int main() {
  M a;
  M r = from_rref(static_cast<M&&>(a));
  Sink s = convert();
  M c = from_lref(a);
  M l = local(true);
  printf(".\n");
}
