// [basic.scope.pdecl]/7.2: the `struct Q` of a local class's member
// function parameter declares `Q` in the function's block scope, and the
// later definition there redeclares it: every written `Q` names the one
// entity, the class first declared at 11:28 (the written-reference dump
// names a class by its first declaration). A nested block's `struct Q` is
// another class (13:12). WP-2506.
//@ expect: compile-ok
//@ strict-diags
//@ written-refs: elaborated-type-local-class-member-refs.refs
void f() {
  struct A { void g(struct Q *) {} struct Q *pq; };
  struct Q { long m; };
  { struct Q { int z; }; Q inner{1}; (void)inner; }
  A a;
  Q q;
  a.g(&q);
  a.pq = &q;
}
