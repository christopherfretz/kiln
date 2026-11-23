// Lookup provenance (spec/ast.md §18.7.2): a name whose lookup failed
// and a dependent name have no entity and `how` none. A failed call
// whose argument-dependent lookup ran keeps its set (u and u::S,
// [basic.lookup.argdep]/3.2); a dependent call in a pattern performs it
// at instantiation and has none (§18.7.4). The argument's type `S` is
// found through the file-scope directive.
//@ expect: compile-fail
//@ written-refs: provenance-unresolved.refs
//@ diag: 15:10 error "use of undeclared identifier 'missing'"
namespace u {
struct S {};
}
using namespace u;
int f() {
  return missing(S{});
}
template <class T> int g(T t) { return dep(t) + T::value; }
