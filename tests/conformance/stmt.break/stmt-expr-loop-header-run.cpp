// [stmt.break]/1, [stmt.cont]/1, [stmt.while], [stmt.do], [stmt.for],
// [stmt.ranged], [stmt.switch] (GNU statement expressions): a break or
// continue in a statement expression in a loop or switch header targets the
// loop that is open there, as clang's codegen does. while: from its
// condition (continue re-evaluates the condition). do: from its body to the
// end of its condition (continue re-evaluates the condition). for: from its
// condition through its increment (continue goes to the increment, or to
// the condition when there is none). A for loop's init-statement, a
// range-based for's init and range, and a switch's condition belong to the
// enclosing loop. Each function returns a number that tells the targets
// apart. GCC rejects every one of these jumps (an oracle disagreement).
// WP-1075; spec/sema.md §7.4a.
//@ expect: run
//@ stdout: 2 205 3 507 4 506 5 306 1 36 36 33 11 121 16
//@ stdout: 404 2 3 26 404
//@ stdout: 2 506 121 507 3 3 5
extern "C" int printf(const char*, ...);
int while_cond_break() { int n = 0; while (({ if (n == 2) break; n < 10; })) ++n; return n; }
int while_cond_continue() { int n = 0, c = 0; while (({ ++c; if (c < 3) continue; n < 2; })) ++n; return n * 100 + c; }
int do_cond_break() { int n = 0; do { ++n; } while (({ if (n == 3) break; true; })); return n; }
int do_cond_continue() { int n = 0, c = 0; do { ++n; } while (({ ++c; if (c < 3) continue; n < 5; })); return n * 100 + c; }
int for_cond_break() { int n = 0; for (;({ if (n == 4) break; true; }); ++n) {} return n; }
int for_cond_continue() { int n = 0, c = 0; for (; ({ ++c; if (c < 3) continue; n < 5; }); ++n) {} return n * 100 + c; }
int for_inc_break() { int n = 0; for (;; ({ if (n == 5) break; ++n; })) {} return n; }
int for_inc_continue() { int n = 0, c = 0; for (; n < 3; ({ ++c; if (c % 2) continue; ++n; })) {} return n * 100 + c; }
int for_init_break() { int r = 0; for (int o = 0; o < 3; ++o) { for ((void)({ if (o == 1) break; 0; }); r < 100; ) { ++r; break; } } return r; }
int nested_inner_cond_break() { int r = 0; for (int o = 0; o < 3; ++o) { int i = 0; while (({ if (i == 2) break; true; })) { ++i; ++r; } r += 10; } return r; }
int nested_inner_for_cond_break() { int r = 0; for (int o = 0; o < 3; ++o) { for (int i = 0; ({ if (i == 2) break; true; }); ++i) { ++r; } r += 10; } return r; }
int nested_inner_for_cond_continue() { int r = 0; for (int o = 0; o < 3; ++o) { int c = 0; for (int i = 0; ({ ++c; if (c == 1) continue; i < 2; }); ++i) { ++r; } r += 10; } return r; }
int switch_cond_break() { int r = 0; for (int o = 0; o < 3; ++o) { switch (({ if (o == 1) break; o; })) { case 0: r += 1; break; default: r += 100; } r += 10; } return r; }
int switch_cond_continue() { int r = 0; for (int o = 0; o < 3; ++o) { switch (({ if (o == 1) continue; o; })) { case 0: r += 1; break; default: r += 100; } r += 10; } return r; }
int range_init_break() { int r = 0; int a[3] = {1,2,3}; for (int o = 0; o < 3; ++o) { for (int x : *({ if (o == 1) break; &a; })) r += x; r += 10; } return r; }
int for_inc_continue2() { int conds = 0, incs = 0, n = 0; for (; ({ ++conds; n < 3; }); ({ ++incs; if (incs == 1) continue; ++n; })) {} return conds * 100 + incs; }
int while_condvar_break() { int n = 0; while (bool b = ({ if (n == 2) break; true; })) ++n; return n; }
int for_condvar_break() { int n = 0; for (; bool b = ({ if (n == 3) break; true; }); ++n) {} return n; }
int do_nested() { int r = 0; for (int o = 0; o < 2; ++o) { int i = 0; do { ++i; ++r; } while (({ if (i == 3) break; true; })); r += 10; } return r; }
constexpr int cw() { int n = 0; while (({ if (n == 2) break; true; })) ++n; return n; }
constexpr int cf() { int n = 0, c = 0; for (; ({ ++c; if (c < 3) continue; n < 5; }); ++n) {} return n * 100 + c; }
constexpr int cs() { int r = 0; for (int o = 0; o < 3; ++o) { switch (({ if (o == 1) continue; o; })) { case 0: r += 1; break; default: r += 100; } r += 10; } return r; }
constexpr int cd() { int n = 0, c = 0; do { ++n; } while (({ ++c; if (c < 3) continue; n < 5; })); return n * 100 + c; }
constexpr int ci() { int n = 0; for (;; ({ if (n == 3) break; ++n; })) {} return n; }
constexpr int cr() { int a[2] = {1, 2}, r = 0; for (int o = 0; o < 2; ++o) for (int x : *({ if (o == 1) break; &a; })) r += x; return r; }
constexpr int ok() { int n = 0; while (({ int k = 0; for (;;) { if (++k == 2) break; } n < k + 3; })) ++n; return n; }
int main() {
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", while_cond_break(), while_cond_continue(), do_cond_break(), do_cond_continue(), for_cond_break(), for_cond_continue(), for_inc_break(), for_inc_continue(), for_init_break(), nested_inner_cond_break(), nested_inner_for_cond_break(), nested_inner_for_cond_continue(), switch_cond_break(), switch_cond_continue(), range_init_break());
  printf("%d %d %d %d %d\n", for_inc_continue2(), while_condvar_break(), for_condvar_break(), do_nested(), for_inc_continue2());
  printf("%d %d %d %d %d %d %d\n", cw(), cf(), cs(), cd(), ci(), cr(), ok());
}
