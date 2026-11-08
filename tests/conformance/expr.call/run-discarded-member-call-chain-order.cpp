// [expr.call]/7, [class.temporary]/4: discarded member calls on class
// prvalues whose results have non-trivial destructors: chained calls on
// a returned temporary, a call through a returned reference, arguments
// that are temporaries, a nested call in an argument, a comma and a
// conditional. Every temporary is created once, in evaluation order,
// and destroyed in reverse order at the end of its full-expression
// (WP-774; the order is clang 19's).
//@ expect: run
//@ strict-diags
//@ stdout: M E0 call E1 ~E1 ~E0 ~M |1
//@ stdout: M a M b E2 ~E2 ~M ~M |2
//@ stdout: M r b E2 ~E2 ~M |3
//@ stdout: M E5 E6 i ~E6 ~E5 ~M |4
//@ stdout: mk M E7 call E1 ~E1 ~E7 ~M |5
//@ stdout: M b E2 ~E2 ~M |6
//@ stdout: M a M a M E8 M E9 E4 i call E1 ~E1 ~E4 ~E9 ~M ~E8 ~M ~M ~M |7
//@ stdout: M b E2 M b E2 ~E2 ~M ~E2 ~M |8
//@ stdout: M b E2 ~E2 ~M |9
extern "C" int printf(const char *, ...);
struct E {
  int id;
  E(int i = 0) : id(i) { printf("E%d ", id); }
  E(const E &o) : id(o.id + 10) { printf("Ec%d ", id); }
  ~E() { printf("~E%d ", id); }
};
struct M {
  M() { printf("M "); }
  M(const M &) { printf("Mc "); }
  ~M() { printf("~M "); }
  E e(E, int) && { printf("call "); return E{1}; }
  M a() { printf("a "); return M{}; }
  M &r() { printf("r "); return *this; }
  E b() { printf("b "); return E{2}; }
  int i(E, E) { printf("i "); return 0; }
};
M mk() { printf("mk "); return {}; }
int main() {
  M{}.e(E{}, 3); printf("|1\n");
  M{}.a().b(); printf("|2\n");
  M{}.r().b(); printf("|3\n");
  M{}.i(E{5}, E{6}); printf("|4\n");
  mk().e(E{7}, 1); printf("|5\n");
  (void)M{}.b(); printf("|6\n");
  M{}.a().a().e(E{8}, M{}.i(E{9}, E{4})); printf("|7\n");
  M{}.b(), M{}.b(); printf("|8\n");
  true ? M{}.b() : E{3}; printf("|9\n");
}
