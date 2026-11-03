// [temp.expl.spec]/17-18, [class.mem.general]/11: the default member
// initializer of `Y<int>::M<int>` comes from the explicit specialization
// of the member class template, not the primary member; its error is
// reported there and, as for a member partial specialization
// (member-partial-spec-nsdmi-note.cpp, where clang agrees), the
// instantiation note points at the class name.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 19.1.7 and 22 instantiate the primary member template's initializer (`U u = 0;`, valid) for an explicitly specialized member class template, so they accept this
//@ diag: 17:66 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 17:47 note "in instantiation of default member initializer 'Y<int>::M<int>::u' requested here"
//@ diag: 18:26 note "in evaluation of exception specification for 'Y<int>::M<int>::M' needed here"
template <class T> struct Y {
  template <class U> struct M { U u = 0; };
};

template <> template <class U> struct Y<int>::M { int pad; U u = U::nope; };
int f() { Y<int>::M<int> m; return m.u; }
