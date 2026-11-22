//@ expect: ast
//@ ast-filter: test_
//@ oracle: clang
//@ requires: clang>=22
// [basic.lookup.argdep]/1-4 (spec/sema-overload.md §2.2): an unqualified
// call whose selected function only argument-dependent lookup found is
// marked `adl` (clang `IsADLCandidate`): a function, a function template,
// a hidden friend (also of a class template specialization), an
// unqualified template-id that names no ordinary template ([temp.names]/3),
// a function reached by a using-declaration in an associated namespace,
// an ADL function beating a non-viable or worse ordinary one, a call in a
// constant expression (its value is unchanged) and in a non-template
// requires-expression. Not `adl`: a function ordinary lookup also found
// (through a using-declaration, also a block-scope one; a function
// template in the enclosing namespace) and a parenthesized callee (ADL suppressed, /3.1). Calls in
// templates: adl-call-flag-instantiation-ast.cpp.
namespace N {
struct S {
  friend int hidden(S) { return 1; }
};
int plain(S) { return 2; }
template <class T> int tmpl(T) { return 3; }
int both(S) { return 4; }
int over(S) { return 5; }
template <class T> int tid(T) { return 6; }
constexpr int ce(S) { return 7; }
int viaus(S) { return 8; }
int pt(S) { return 10; }
template <class T> int both_t(T) { return 11; }
template <class T> struct W {
  friend int wf(W) { return 12; }
};
} // namespace N
namespace U {
using N::viaus;
}
namespace M {
struct Q {};
using N::viaus;
int viaus(Q) { return 9; }
} // namespace M
using N::both;
int over(long) { return 0; }
template <class T> int pt(T) { return 0; }
int test_plain(N::S s) { return plain(s); }
int test_tmpl(N::S s) { return tmpl(s); }
int test_hidden(N::S s) { return hidden(s); }
int test_hidden_tmpl(N::W<int> w) { return wf(w); }
int test_both(N::S s) { return both(s); }
namespace N {
int test_both_t(N::S s) { return both_t(s); }
} // namespace N
int test_over(N::S s) { return over(s); }
int test_paren(N::S s) { return (pt)(s); }
int test_pt(N::S s) { return pt(s); }
int test_using(N::S s) {
  using U::viaus;
  return viaus(s);
}
int test_using_ns(M::Q q) { return viaus(q); }
int test_tid(N::S s) { return tid<N::S>(s); }
constexpr int test_ce = ce(N::S{});
static_assert(test_ce == 7);
bool test_req = requires(N::S s) { plain(s); };
