// [over.over]/1, [class.mfct.non.static]/3: an unmatched implicit member
// overload set is diagnosed with every candidate noted. clang finds the
// name in an inline member body through the class scope still open around
// it (the last declared first) and out of line through the class's lookup
// table (in declaration order), so the notes are in that order.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:33 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 25:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 24:8 note "candidate function"
//@ diag: 29:35 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 24:8 note "candidate function"
//@ diag: 25:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 35:35 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 31:8 note "candidate function"
//@ diag: 32:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 40:34 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 38:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 37:8 note "candidate function"
//@ diag: 42:36 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 37:8 note "candidate function"
//@ diag: 38:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
struct E {
  void g(int);
  static void g(long);
  void bad() { void (*p)(int) = g; (void)p; }
  void bad2();
};
void E::bad2() { void (*p)(int) = g; (void)p; }
struct F {
  void g(int);
  static void g(long);
  void bad2();
};
void F::bad2() { void (*p)(int) = g; (void)p; }
struct G {
  void g(int);
  static void g(long);
  void bad2();
  void bad() { void (*p)(int) = &g; (void)p; }
};
void G::bad2() { void (*p)(int) = &g; (void)p; }
