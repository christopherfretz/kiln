// [stmt.return]/4: -Wreturn-type 'always' vs 'maybe' vs silent per clang's
// CheckFallThrough over the pruned CFG: loops, goto, noreturn calls, throw,
// try dispatch without a catch-all, lambdas and templates (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-deprecated-declarations
//@ diag: 29:46 warning "non-void function does not return a value in all control paths"
//@ diag: 30:80 warning "non-void function does not return a value in all control paths"
//@ diag: 32:86 warning "non-void function does not return a value in all control paths"
//@ diag: 33:48 warning "non-void function does not return a value in all control paths"
//@ diag: 38:36 warning "non-void function does not return a value"
//@ diag: 41:33 warning "non-void function does not return a value"
//@ diag: 44:42 warning "non-void function does not return a value in all control paths"
//@ diag: 46:62 warning "non-void function does not return a value"
//@ diag: 49:45 warning "function declared 'noreturn' should not return"
//@ diag: 52:60 warning "function declared 'noreturn' should not return"
//@ diag: 53:47 warning "non-void lambda does not return a value in all control paths"
//@ diag: 54:30 warning "non-void lambda does not return a value"
//@ diag: 57:83 warning "non-void function does not return a value in all control paths"
//@ diag: 64:51 warning "non-void function does not return a value in all control paths"
//@ diag: 65:11 note "in instantiation of function template specialization 'tt<int>' requested here"
//@ requires: clang<22
// clang 22 emits warning: enumeration value 'C' not handled in switch; WP-355 residual
void g();
[[noreturn]] void ab();
int ret();
enum E { A, B, C [[deprecated]] };
enum M { MA, MB [[maybe_unused]] };
int v1() { return 1; try { } catch (int) { } }
int v2() { try { try { throw 1; } catch (long) { } } catch (int) { return 1; } }
int v3() { try { try { throw 1; } catch (long) { return 2; } } catch (...) { return 1; } }
int v4() { try { try { g(); } catch (long) { return 2; } } catch (int) { return 1; } }
int v5() { try { g(); } catch (int) { throw; } }
int v6() { try { return 1; } catch (int) { throw; } }
int v7(E e) { switch (e) { case A: return 1; case B: return 2; } }
int v8(M m) { switch (m) { case MA: return 1; } }
int v9(E e) { switch (e) { case A: return 1; case B: return 2; case C: return 3; default: break; } }
int v10(int x) { l: if (x) goto l; }
int v11(int x) { l: if (x--) goto l; return 1; }
int v12() { for (;;) { } }
int v13() { for (;;) { break; } }
int v14(int x) { do { return 1; } while (x); }
int v15() { while (1) { g(); } }
int v16(int x) { while (x) { return 1; } }
int v17() { switch (1) { case 1: return 1; default: break; } }
int v18() { switch (4) { case 1: return 1; default: break; } }
int v19(int x) { return x ? 1 : throw 2; }
int v20(int x) { if (x) return 1; else ab(); }
[[noreturn]] void v21(int x) { if (x) ab(); }
[[noreturn]] void v22() { while (true) {} }
[[noreturn]] void v23() { throw 1; }
[[noreturn]] void v24() { try { throw 1; } catch (int) { } }
auto l1 = [](int x) -> int { if (x) return 1; };
auto l2 = [](int x) -> int { };
int v25(E e) { switch (e) { case A: return 1; case B: return 2; default: return 3; } }
int v26(E e) { switch (e) { case A: g(); default: return 3; case B: return 2; } }
int v27(int x) { try { g(); } catch (int) { return 1; } catch (...) { return 2; } }
int v28() { try { throw 1; } catch (int) { return 1; } }
int v29() { if (false) { try { } catch (int) { } } return 1; }
int v30() { try { ab(); } catch (int) { return 1; } }
int v31(int x) { try { if (x) return 1; ab(); } catch (int) { return 1; } }
int v32() { try { [] { throw 1; }(); } catch (int) { } return 1; }
int v33() { try { g(); } catch (int) { } return 1; }
template <class T> int tt(T t) { if (t) return 1; }
int v34 = tt(1);
