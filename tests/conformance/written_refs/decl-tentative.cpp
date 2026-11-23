// The written-reference table (spec/ast.md §18.5) in declarative
// contexts (stage 3): disambiguation scans tokens without recording, so
// the kept parse records each name once. A parenthesized qualified
// declarator-id (`int (Counter::total) = 0;`, [dcl.ambig.res]/1) and a
// statement that is a declaration (`Counter::Unit(u);`, [stmt.ambig]/1)
// against one that is an expression whose argument has designators.
//@ written-refs: decl-tentative.refs
//@ args: -Wno-vexing-parse
struct Counter {
  static int total;
  struct Unit {};
  static int add(Unit);
};
struct Opts {
  int level;
};
int consume(Opts);
int (Counter::total) = 0;

void run() {
  Counter::Unit(u);
  consume({.level = 1});
  Counter::add(u);
}
