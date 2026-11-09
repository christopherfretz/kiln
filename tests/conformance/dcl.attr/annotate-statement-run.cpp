// clang's `annotate` on a statement (WP-954): clang 22 accepts it there
// (`StmtAttr`) and the statement runs as without it; clang 19 rejects it
// (annotate-statement-19.cpp). [dcl.attr.grammar]/5, [stmt.pre]/1.
//@ requires: clang>=22
//@ std: c++20
//@ expect: run
//@ strict-diags
int f(int n) {
  int s = 0;
  [[clang::annotate("loop", 1)]] for (int i = 0; i < n; ++i) s += i;
  [[clang::annotate("ret")]] return s;
}
int main() { return f(4) == 6 ? 0 : 1; }
