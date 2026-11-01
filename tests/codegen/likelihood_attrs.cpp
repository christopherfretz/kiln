// [dcl.attr.likelihood], [dcl.attr.fallthrough] (WP-182): at -O0 clang 19
// emits no branch weights (`!prof`) or `llvm.expect` for `[[likely]]` /
// `[[unlikely]]` (CodeGenFunction::EmitIfStmt and the switch lowering only
// consult likelihood when optimizing), and `[[fallthrough]]` emits nothing.
int g(int);
int f(int x) {
  if (x > 0) [[likely]] { g(1); } else [[unlikely]] g(2);
  switch (x) {
  [[likely]] case 1: g(3); [[fallthrough]];
  case 2: g(4); break;
  [[unlikely]] default: break;
  }
  while (x) [[likely]] --x;
  for (int i = 0; i < 3; ++i) [[unlikely]] g(i);
  do [[likely]] { --x; } while (x > -3);
  [[unlikely]] return x;
}
template <class T> T t(T x) {
  switch (x) {
  case 1: [[fallthrough]];
  [[unlikely]] case 2: return x;
  }
  if (x) [[likely]] return 1;
  return 0;
}
int h() { return t(1) + int(t(2L)); }
