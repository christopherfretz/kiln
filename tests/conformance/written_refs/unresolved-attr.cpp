// Unresolvable thread-safety attribute arguments (spec/ast.md §18.8,
// WP-2291): kiln parses them for their names alone (WP-2170) and gives no
// diagnostic, where clang rejects an undeclared identifier there; the
// name is `UNRESOLVED` like any failed lookup, and so is a member name
// after it. The forms clang accepts with a -Wthread-safety-attributes
// warning (a variable whose type is not a capability, pt_guarded_by of a
// non-pointer) record what lookup found; a string literal has no entry; a
// dependent argument is `DEPENDENT`.
//@ expect: compile-fail
//@ written-refs: unresolved-attr.refs
//@ diag: 26:9 error "use of undeclared identifier 'undeclared'"
#define TS(x) __attribute__((x))
struct TS(capability("mutex")) Mutex {
  int x;
};
int plain;
Mutex mu;
int a TS(guarded_by(nomu));
int b TS(guarded_by(nomu2.x));
int c TS(guarded_by(plain));
int d TS(guarded_by("mu"));
int e TS(pt_guarded_by(mu));
template <class T> struct S {
  int f TS(guarded_by(T::mu));
};
int z = undeclared;
