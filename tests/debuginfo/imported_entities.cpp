// Debugger (WP-1365, reported by hod; [namespace.udir], [namespace.udecl],
// [namespace.alias]): names reached through a using-directive in a
// namespace (`secret` inside app::run), a block-scope using-directive
// (`origin` in main), a block-scope using-declaration (`scale(2)` calls
// through it), a block-scope namespace alias (`d::secret`) and a
// namespace-scope alias (`g::origin`) resolve in gdb and lldb, which look
// them up through DW_TAG_imported_module / DW_TAG_imported_declaration.
//@ expect: debugger
//@ gdb: break app::run
//@ gdb: break imported_entities.cpp:64
//@ gdb: run
//@ gdb: print secret
//@ gdb: continue
//@ gdb: print origin
//@ gdb: print scale(2)
//@ gdb: print d::secret
//@ gdb: print g::origin
//@ gdb: print total
//@ gdb-check: Breakpoint 1, app::run () at
//@ gdb-check: $1 = 42
//@ gdb-check: Breakpoint 2, main () at
//@ gdb-check: $2 = 10
//@ gdb-check: $3 = 6
//@ gdb-check: $4 = 42
//@ gdb-check: $5 = 10
//@ gdb-check: $6 = 101
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --name app::run
//@ lldb: breakpoint set --file imported_entities.cpp --line 64
//@ lldb: run
//@ lldb: expr secret
//@ lldb: continue
//@ lldb: expr origin
//@ lldb: expr scale(2)
//@ lldb: expr g::origin
//@ lldb: frame variable total
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: (int) $0 = 42
//@ lldb-check: stop reason = breakpoint 2.1
//@ lldb-check: (int) $1 = 10
//@ lldb-check: (int) $2 = 6
//@ lldb-check: (int) $3 = 10
//@ lldb-check: (int) total = 101
namespace geo {
int origin = 10;
int scale(int v) { return v * 3; }
namespace detail {
int secret = 42;
}
} // namespace geo
namespace g = geo;
namespace app {
using namespace geo::detail;
int run() {
  int r = secret + 1;
  return r;
}
} // namespace app
int main() {
  using namespace geo;
  using geo::scale;
  namespace d = geo::detail;
  int total = origin + scale(2) + d::secret + app::run();
  return total == 101 ? 0 : 1;
}
