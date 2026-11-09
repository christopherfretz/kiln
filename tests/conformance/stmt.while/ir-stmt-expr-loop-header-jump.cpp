// [stmt.break]/1, [stmt.cont]/1, [stmt.while], [stmt.do], [stmt.for],
// [stmt.switch] (GNU statement expressions): the IR for a break or continue
// in a statement expression in a loop or switch header branches to the same
// blocks as clang's: while and do conditions (exit; re-evaluate the
// condition), a for condition and increment (exit; the increment), and a
// switch condition (the enclosing loop). WP-1075; spec/sema.md §7.4a.
//@ expect: ir
int g(int);
int while_cond(int n) { while (({ if (g(n)) break; if (g(n + 1)) continue; n < 10; })) ++n; return n; }
int do_cond(int n) { do { ++n; } while (({ if (g(n)) break; if (g(n + 1)) continue; true; })); return n; }
int for_header(int n) { for (; ({ if (g(n)) break; if (g(n + 1)) continue; true; }); ({ if (g(n + 2)) break; if (g(n + 3)) continue; ++n; })) {} return n; }
int switch_cond(int n) { for (int o = 0; o < n; ++o) { switch (({ if (g(o)) break; if (g(o + 1)) continue; o; })) { case 0: n += 1; break; default: n += 2; } } return n; }
