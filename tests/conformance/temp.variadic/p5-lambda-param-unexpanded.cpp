// [temp.variadic]/5: a lambda-expression whose call operator's parameter
// or return type names a pack it does not expand contains an unexpanded
// pack, so the enclosing full-expression is diagnosed (clang:
// "initializer contains unexpanded parameter pack 'Ts'", "expression
// contains ..." for an expression statement), also after the varargs `...`
// of `[](Ts a...)` (-Wambiguous-ellipsis, whose "place '...' here" note
// for an abstract declarator points at its innermost position). A pack
// expansion around the lambda expands it.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 46:19 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 46:18 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 46:19 note "insert ',' before '...' to silence this warning"
//@ diag: 46:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 47:23 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 47:20 note "place '...' here to declare a function parameter pack"
//@ diag: 47:23 note "insert ',' before '...' to silence this warning"
//@ diag: 47:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 48:10 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 48:9 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 48:10 note "insert ',' before '...' to silence this warning"
//@ diag: 48:3 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 49:16 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 49:15 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 49:16 note "insert ',' before '...' to silence this warning"
//@ diag: 49:3 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 50:25 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 50:24 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 50:25 note "insert ',' before '...' to silence this warning"
//@ diag: 50:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 51:18 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 51:17 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 51:18 note "insert ',' before '...' to silence this warning"
//@ diag: 51:11 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 52:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 55:30 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 55:29 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 55:30 note "insert ',' before '...' to silence this warning"
//@ diag: 55:23 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 57:39 warning "'...' in this location creates a C-style varargs function, not a function parameter pack"
//@ diag: 57:38 note "place '...' immediately before declared identifier to declare a function parameter pack"
//@ diag: 57:39 note "insert ',' before '...' to silence this warning"
//@ diag: 57:32 error "initializer contains unexpanded parameter pack 'Ts'"
template <class... Ts> void l() {
  auto q = [](Ts a...) {};
  auto r = [](Ts (*)()...) {};
  [](Ts a...) {};
  (void)[](Ts a...) {};
  auto t = [](int, Ts *a...) {};
  int w = [](Ts a...) { return 0; }(1);
  auto x = [](int) -> Ts { return {}; };
}
template <class... Ts> struct C {
  void m() { auto q = [](Ts a...) {}; }
};
template <class... Ts> int v = [](Ts a...) { return 0; }(0);
// The lambda's own pack, or one expanded around the lambda: no error.
template <class... Ts> int ok(Ts... ts) {
  int r = 0;
  ((r += [](Ts a) { return (int)a; }(ts)), ...);
  auto s = [](Ts... a) { return sizeof...(a); };
  return r + (int)s(ts...);
}
int use = ok(1, 2L);
