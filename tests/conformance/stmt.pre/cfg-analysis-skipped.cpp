// [stmt.pre], [implimits]: a body whose control-flow graph would hold more
// elements than -fkiln-cfg-max-elems gets none; its flow-sensitive warnings
// (here -Wreturn-type and -Wmissing-noreturn) are skipped, reported once
// per body by the kiln-only remark -Rkiln-analysis-skipped when a consumer
// first asks for the graph (spec/sema.md §7.5). A small body still warns.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fkiln-cfg-max-elems=10 -Rkiln-analysis-skipped -Wmissing-noreturn
//@ skip(clang): kiln-only flags
//@ diag: 14:5 remark "flow-sensitive warnings for 'big' skipped: its control-flow graph exceeds -fkiln-cfg-max-elems=10"
//@ diag: 20:1 warning "non-void function does not return a value in all control paths"
//@ diag: 22:17 remark "flow-sensitive warnings for 'n::S::m' skipped: its control-flow graph exceeds -fkiln-cfg-max-elems=10"
[[noreturn]] void die();
int big(int a) {
  int b = a + 1, c = b + 2, d = c + 3;
  if (a) return b + c + d;
}
int small(int a) {
  if (a) return 1;
}
namespace n {
struct S { void m(int a) { int b = a, c = b, d = c, e = d; (void)e; die(); } };
} // namespace n
