// [stmt.return]/2, [except.handle]/16: flowing off the end of a handler of
// a value-returning function; a throw-expression statement does not flow
// off ([expr.throw]).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:45 warning "non-void function does not return a value in all control paths"
//@ diag: 11:45 warning "non-void function does not return a value in all control paths"
//@ diag: 13:41 warning "non-void function does not return a value in all control paths"
int a() { try { return 1; } catch (...) { return 2; } }
int b() { try { return 1; } catch (...) { } }
int c() { try { } catch (...) { return 2; } }
int d() { try { throw 1; } catch (...) { throw; } }
int e() try { return 1; } catch (...) { }
int g() { throw 1; }
