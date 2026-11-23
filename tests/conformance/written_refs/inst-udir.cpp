// Per-specialization entries (spec/ast.md §18.9.1, WP-2220): the ordinary
// lookup of a dependent call's name runs at the template's definition
// ([temp.dep.candidate], [temp.res.general]/1), so a row of a
// specialization that selects a declaration ordinary lookup found through
// a using-directive ([namespace.udir]/2, [namespace.qual]/2) names that
// directive (`udir=`), as a main-table entry does (§18.7.3). A function
// found only by argument-dependent lookup names none: it ignores
// using-directives ([basic.lookup.argdep]/4).
//@ written-refs: inst-udir.refs
namespace a {
struct S {};
void f(int);
template <class T> void h(T);
void both(S);
} // namespace a
namespace b {
void g(int);
void g(long);
} // namespace b
namespace c {
using namespace b;
void k(int);
void k(long);
} // namespace c
namespace d {
void m(int);
} // namespace d
namespace e {
using namespace d;
} // namespace e
namespace q {
struct R {};
void adl_only(R);
} // namespace q

using namespace a;

template <class T> void run(T t) {
  f(t);    // through `using namespace a;` at file scope
  h(t);    // a function template: its specialization, the same directive
  c::g(t); // qualified (an overload set), through `using namespace b;` in `c`
  c::k(t); // `c`'s own member: no directive
}

// Both lookups find `a::both`: ordinary lookup's directive is kept.
template <class T> void both_ways(T t) { both(t); }

// Argument-dependent lookup alone finds `q::adl_only`: no directive.
template <class T> void adl(T t) { adl_only(t); }

template <class T> void block(T t) {
  using namespace e; // a block-scope directive, transitive to `d`
  m(t);
}

void use() {
  run(1);
  run(2L);
  both_ways(S{});
  adl(q::R{});
  block(3);
}
