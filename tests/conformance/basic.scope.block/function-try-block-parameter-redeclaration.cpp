// [basic.scope.block]/2: the compound statement of a function-try-block
// and each handler's exception-declaration and outermost block cannot
// redeclare a parameter. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 15:25 error "redefinition of 'p'"
//@ diag: 15:12 note "previous definition is here"
//@ diag: 16:33 error "redefinition of 'p'"
//@ diag: 16:12 note "previous definition is here"
//@ diag: 17:40 error "redefinition of 'p'"
//@ diag: 17:12 note "previous definition is here"
//@ diag: 19:37 error "redefinition of 'p'"
//@ diag: 19:9 note "previous definition is here"
void a(int p) try { int p; } catch (...) {}
void b(int p) try {} catch (int p) {}
void c(int p) try {} catch (...) { int p; }
struct S {
  S(int p) try {} catch (...) { int p; }
};
// Valid: a nested block may redeclare the parameter.
void d(int p) try { { int p = 0; (void)p; } } catch (...) { { int p = 0; (void)p; } }
