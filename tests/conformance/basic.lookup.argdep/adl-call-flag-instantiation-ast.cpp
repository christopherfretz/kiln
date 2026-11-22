//@ expect: ast
//@ ast-filter: inst_
//@ oracle: none
//@ requires: clang>=22
//@ skip(clang): the clang dump normalizer does not print function template specializations yet (spec/ast-dump.md §5)
// [basic.lookup.argdep], [temp.dep.candidate] (spec/sema-overload.md
// §2.2): in a template, a call with a dependent argument is resolved at
// instantiation, where the function only argument-dependent lookup found
// marks the instantiated call `adl` (clang `IsADLCandidate`); a
// non-dependent call is resolved at the definition, which keeps the flag,
// and its instantiation is rebuilt from the resolved callee without it, as
// clang's is. `oracle: none`: the clang dump normalizer does not print
// specializations yet (spec/ast-dump.md §5); the golden's `adl` markers
// match clang 19 and 22's `-ast-dump`.
namespace N {
struct S {};
int plain(S) { return 2; }
template <class T> int tmpl(T) { return 3; }
} // namespace N
template <class T> int inst_dep(T t) { return plain(t) + tmpl(t); }
template <class T> int inst_nondep(T) {
  N::S s;
  return plain(s);
}
int inst_use(N::S s) { return inst_dep(s) + inst_nondep(0); }
