// Per-specialization entries (spec/ast.md §18.9.3): rows belong to the
// specializations that are formed. At clang compat 22 (CWG2369 as clang
// 21 implements it, overload/defer.rs) a function template candidate's
// deduction is deferred while a non-template candidate is a perfect
// match; that match wins, the template is never deduced, no
// specialization of it is formed, and its signature's dependent name
// (`T::s`) gets no row (`r`). When the non-template candidate is not a
// perfect match (`int` to `short`), the template's deduction runs
// ([temp.over]/1), forms `p<n::S>` and keeps the row of its signature,
// although overload resolution then prefers the non-template function
// ([over.match.best]/2.4).
//@ requires: clang>=22
//@ written-refs: inst-deferred-candidate.refs
namespace n {
struct S { static int s(); };
} // namespace n

template <class T> auto r(T, long) -> decltype(T::s());
int r(n::S, int);
int use_r() { return r(n::S(), 1); }

template <class T> auto p(T, long) -> decltype(T::s());
int p(n::S, short);
int use_p() { return p(n::S(), 1); }
