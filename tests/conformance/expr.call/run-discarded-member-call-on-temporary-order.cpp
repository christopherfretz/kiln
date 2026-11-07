// [expr.call]/7, [class.temporary]/4: in a discarded member call on a
// temporary, `M{}.e(E{}, 3);`, the object expression is sequenced before
// the arguments, the result temporary is created by the call, and all
// three temporaries are destroyed in reverse order of construction at
// the end of the full-expression (WP-774).
//@ expect: run
//@ strict-diags
//@ stdout: M E call E ~E ~E ~M end
extern "C" int printf(const char *, ...);
struct E {
  E() { printf("E "); }
  E(const E &) { printf("Ec "); }
  ~E() { printf("~E "); }
};
struct M {
  M() { printf("M "); }
  ~M() { printf("~M "); }
  E e(E, int) && {
    printf("call ");
    return E{};
  }
};
int main() {
  M{}.e(E{}, 3);
  printf("end\n");
}
