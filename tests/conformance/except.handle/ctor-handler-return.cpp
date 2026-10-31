// [except.handle]/13: a return statement in a handler of the
// function-try-block of a constructor is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:35 error "return in the catch of a function try block of a constructor is illegal"
//@ diag: 10:41 error "return in the catch of a function try block of a constructor is illegal"
struct S {
  int m;
  S() try : m(1) {} catch (...) { return; }
  S(int) try {} catch (int) { if (true) return; }
  ~S() try {} catch (...) { return; }
};
int f() try { return 1; } catch (...) { return 2; }
