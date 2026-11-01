// [except.handle] + [stmt.return]/4: a live try dispatch with no catch-all is an
// abnormal edge to exit, turning 'always' fall-off into 'maybe' (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 23:41 warning "non-void function does not return a value in all control paths"
//@ diag: 24:41 warning "non-void function does not return a value"
//@ diag: 25:46 warning "non-void function does not return a value in all control paths"
//@ diag: 26:45 warning "non-void function does not return a value in all control paths"
//@ diag: 27:45 warning "non-void function does not return a value"
//@ diag: 28:51 warning "non-void function does not return a value in all control paths"
//@ diag: 30:67 warning "non-void function does not return a value in all control paths"
//@ diag: 31:30 warning "non-void function does not return a value in all control paths"
//@ diag: 32:34 warning "non-void function does not return a value in all control paths"
//@ diag: 34:34 warning "non-void function does not return a value in all control paths"
//@ diag: 35:18 warning "non-void function does not return a value"
//@ diag: 36:20 warning "expression result unused"
//@ diag: 36:34 warning "non-void function does not return a value in all control paths"
//@ diag: 37:69 warning "non-void function does not return a value in all control paths"
//@ diag: 38:62 warning "non-void function does not return a value"
//@ diag: 39:52 warning "non-void function does not return a value in all control paths"
void g();
[[noreturn]] void ab();
int t1() { try { g(); } catch (int) { } }
int t2() { try { g(); } catch (...) { } }
int t3() { try { return 1; } catch (int) { } }
int t4() { try { throw 1; } catch (int) { } }
int t5() { try { throw 1; } catch (...) { } }
int t6() { try { g(); } catch (int) { return 2; } }
int t7() { try { return 1; } catch (int) { return 2; } }
int t8(int x) { try { if (x) throw 1; return 1; } catch (int) { } }
int t9(int x) { if (x) ab(); }
int t10(int x) { if (x) throw 1; }
int t11(int x) { x ? ab() : ab(); }
int t12(int x) { x ? ab() : g(); }
int t13() { g(); }
int t14(int x) { x && (ab(), 1); }
int t15(int x) { try { try { g(); } catch (...) {} } catch (int) {} }
int t16(int x) { try { if (0) throw 1; g(); } catch (int) {} }
int t17(int x) { try { } catch (int) { return 1; } }
