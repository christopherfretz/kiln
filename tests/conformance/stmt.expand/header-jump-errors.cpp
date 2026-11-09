// [stmt.expand], [stmt.break]/1, [stmt.cont]/1 (GNU statement
// expressions): an expansion statement is a loop for break/continue only in
// its body. A jump in a statement expression in its header (init-statement,
// expansion-init-list element, destructuring or iterating initializer)
// needs an enclosing loop (or switch, for break). It is diagnosed at the
// keyword, also in a template definition. A lambda in the header starts a
// new function. An outer expansion statement does count as the loop. An
// iterating range is a constexpr variable (P1306R5 [stmt.expand]/5.2), so a
// break in it is not a constant expression (GCC 16.1 accepts it: CWG3131,
// see spec/sema.md §7.6a). clang 22 has no P1306; GCC 16.1 and clang's
// P1306 branch give the same accept/reject verdicts and the same
// locations. header-jump-cfg.cpp has the valid jumps' flow. WP-1081;
// spec/sema.md §7.6a.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 40:28 error "'break' statement not in loop or switch statement"
//@ diag: 41:28 error "'continue' statement not in loop statement"
//@ diag: 42:30 error "'break' statement not in loop or switch statement"
//@ diag: 43:33 error "'continue' statement not in loop statement"
//@ diag: 44:29 error "'break' statement not in loop or switch statement"
//@ diag: 50:30 error "'continue' statement not in loop statement"
//@ diag: 51:32 error "'continue' statement not in loop statement"
//@ diag: 52:31 error "'continue' statement not in loop statement"
//@ diag: 58:34 error "'break' statement not in loop or switch statement"
//@ diag: 59:32 error "'continue' statement not in loop statement"
//@ diag: 64:36 error "'break' statement not in loop or switch statement"
//@ diag: 65:26 error "'continue' statement not in loop statement"
//@ diag: 77:28 error "constexpr variable '__range' must be initialized by a constant expression"
//@ diag: 77:31 note "this use of statement expressions is not supported in a constant expression"
struct P { int a, b; };
struct Arr {
  int v[2];
  constexpr const int* begin() const { return v; }
  constexpr const int* end() const { return v + 2; }
};

// No enclosing loop or switch.
void no_loop() {
  template for (int i = ({ break; 0; }); auto x : {1, 2}) {}
  template for (int i = ({ continue; 0; }); auto x : {1, 2}) {}
  template for (auto x : {({ break; 1; }), 2}) {}
  template for (auto x : {1, ({ continue; 2; })}) {}
  template for (auto x : ({ break; P{1, 2}; })) {}
}
// Only a switch around: continue still has no loop.
void only_switch(int n) {
  switch (n) {
  case 1:
    template for (int i = ({ continue; 0; }); auto x : {1}) {}
    template for (auto x : {({ continue; 1; })}) {}
    template for (auto x : ({ continue; P{1, 2}; })) {}
  }
}
// A lambda in the header starts a new function.
void in_lambda() {
  for (;;) {
    template for (auto x : {[] { break; return 1; }()}) {}
    template for (int i = [] { continue; return 0; }(); auto x : {1}) {}
  }
}
// A template definition: diagnosed at definition time.
template <class T> void in_template() {
  template for (auto x : {T(1), ({ break; T(2); })}) {}
  template for (T i = ({ continue; T(); }); auto x : {1}) {}
}
// Nested: the outer expansion statement is the loop (no diagnostic).
void nested() {
  template for (auto y : {1, 2}) {
    template for (int i = ({ continue; 0; }); auto x : {1}) {}
    template for (auto x : ({ break; P{1, 2}; })) {}
  }
}
// Iterating: valid jump, but the range must be a constant expression.
void iterating() {
  for (;;) {
    template for (auto x : ({ break; Arr{1, 2}; })) {}
  }
}
