// [dcl.asm]/1 (CWG2262): an asm-declaration takes an attribute-specifier-
// seq, but no attribute appertains to it: at block scope each attribute,
// known or unknown, is "'x' attribute ignored" [-Wignored-attributes] at
// its start, its arguments unparsed (clang 22 names a scoped one with its
// scope); a GNU attribute there is "an attribute list cannot appear here"
// (clang 22; GCC 16.1 warns "attributes ignored on 'asm' declaration").
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 25:5 warning "'maybe_unused' attribute ignored"
//@ diag: 26:5 warning "'foo::bar' attribute ignored"
//@ diag: 27:5 warning "'foo::bar' attribute ignored"
//@ diag: 27:26 warning "'bar::baz' attribute ignored"
//@ diag: 28:16 warning "'foo::bar' attribute ignored"
//@ diag: 28:21 warning "'foo::baz' attribute ignored"
//@ diag: 29:5 warning "'nodiscard' attribute ignored"
//@ diag: 29:16 warning "'deprecated' attribute ignored"
//@ diag: 30:3 error "an attribute list cannot appear here"
//@ diag: 31:5 warning "'foo::bar' attribute ignored"
//@ diag: 31:16 error "an attribute list cannot appear here"
void f() {
  int i = 42;
  [[]] asm("");
  [[maybe_unused]] asm("");
  [[foo::bar]] asm("");
  [[foo::bar(1, {2})]] [[bar::baz]] asm("" : "+r"(i));
  [[using foo: bar, baz]] asm volatile("");
  [[nodiscard, deprecated("x")]] asm("");
  __attribute__((unused)) asm("");
  [[foo::bar]] __attribute__((unused)) asm("");
  asm("");
}
