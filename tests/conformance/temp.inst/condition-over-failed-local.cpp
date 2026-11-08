// [stmt.if], [stmt.while], [stmt.for], [stmt.switch], [temp.inst]: a
// condition (or `for` increment) naming a local whose initializer failed to
// instantiate is no new error, and the statement's sub-statements are still
// instantiated and diagnosed: clang's reference to the variable is a
// RecoveryExpr and the condition over it is valid (WP-826; kiln dropped the
// statement, so an error in a branch went missing and a later one took its
// place under the error limit). A failed `then` branch ends the `if`
// (clang's TransformIfStmt): its `else` branch is not instantiated, unless
// the failure is a declaration's initializer, which leaves the statement
// valid.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 42:16 error "no matching function for call to 'g'"
//@ diag: 70:15 note "in instantiation of function template specialization 'if_then<R>' requested here"
//@ diag: 39:5 note "candidate function not viable: no known conversion from 'R' to 'int' for 1st argument"
//@ diag: 43:18 error "no member named 'then_branch' in 'R'"
//@ diag: 46:16 error "no matching function for call to 'g'"
//@ diag: 71:15 note "in instantiation of function template specialization 'if_else<R>' requested here"
//@ diag: 39:5 note "candidate function not viable: no known conversion from 'R' to 'int' for 1st argument"
//@ diag: 47:21 error "no member named 'else_branch' in 'R'"
//@ diag: 50:16 error "no matching function for call to 'g'"
//@ diag: 72:15 note "in instantiation of function template specialization 'loops<R>' requested here"
//@ diag: 39:5 note "candidate function not viable: no known conversion from 'R' to 'int' for 1st argument"
//@ diag: 51:16 error "no member named 'while_body' in 'R'"
//@ diag: 52:17 error "no member named 'for_body' in 'R'"
//@ diag: 53:19 error "no member named 'for_inc_body' in 'R'"
//@ diag: 54:27 error "no member named 'switch_body' in 'R'"
//@ diag: 58:21 error "no member named 'then_fails' in 'R'"
//@ diag: 73:15 note "in instantiation of function template specialization 'then_fails<R>' requested here"
//@ diag: 61:31 error "no member named 'init_fails' in 'R'"
//@ diag: 74:15 note "in instantiation of function template specialization 'then_decl_fails<R>' requested here"
//@ diag: 61:55 error "no member named 'else_instantiated' in 'R'"
//@ diag: 66:24 error "no viable conversion from returned value of type 'int' to function return type 'R'"
//@ diag: 75:12 note "in instantiation of function template specialization 'then_return_fails<R>' requested here"
//@ diag: 38:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const R &' for 1st argument"
//@ diag: 38:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'R &&' for 1st argument"
struct R { };
int g(int);

template <class T> void if_then(T x) {
  unsigned n = g(x);
  if (n == 1) T::then_branch();
}
template <class T> void if_else(T x) {
  unsigned n = g(x);
  if (n) {} else T::else_branch();
}
template <class T> void loops(T x) {
  unsigned n = g(x);
  while (n) T::while_body();
  for (; n;) T::for_body();
  for (;; ++n) T::for_inc_body();
  switch (n) { case 0: T::switch_body(); }
}
// clang's TransformIfStmt stops at a `then` branch that is StmtError.
template <class T> void then_fails(T) {
  if (sizeof(T)) T::then_fails(); else T::not_instantiated();
}
template <class T> void then_decl_fails(T) {
  if (sizeof(T)) { int k = T::init_fails(); } else T::else_instantiated();
}
// clang's BuildReturnStmt: a `return` whose operand does not convert is
// StmtError too.
template <class T> T then_return_fails(int p) {
  if (p == 1) { return 1; } else if (p == 0) { return T::not_instantiated(); }
  return T();
}

template void if_then(R);
template void if_else(R);
template void loops(R);
template void then_fails(R);
template void then_decl_fails(R);
template R then_return_fails<R>(int);
