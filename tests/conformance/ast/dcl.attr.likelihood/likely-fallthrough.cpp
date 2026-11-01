//@ expect: ast
//@ ast-filter: f
// [dcl.attr.likelihood], [dcl.attr.fallthrough] (WP-182): `[[likely]]`,
// `[[unlikely]]` and `[[fallthrough]]` are kept in an AttributedStmt
// around their statement (clang `ProcessStmtAttributes`), including a
// case label, a loop body and a return.
int g(int);
int f(int x) {
  if (x > 0) [[likely]] { g(1); } else [[unlikely]] g(2);
  switch (x) {
  [[likely]] case 1: g(3); [[fallthrough]];
  case 2: g(4); break;
  [[unlikely]] default: break;
  }
  while (x) [[likely]] --x;
  for (;;) [[unlikely]] break;
  [[unlikely]] return 0;
}
template <class T> T tm(T x) {
  switch (x) {
  case 1: [[fallthrough]];
  [[unlikely]] case 2: return x;
  }
  if (x) [[likely]] return 1;
  return 0;
}
int h() { return tm(1); }
