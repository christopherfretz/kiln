// [temp.param]/1: a concept-tt-parameter is introduced by
// `template < template-parameter-list > concept`, not by a template-head, so
// it has no requires-clause and no associated constraints; kiln diagnoses
// one. A variable-tt-parameter's template-head may carry a requires-clause.
// clang 22 accepts the concept form silently, so this follows the wording
// (spec/sema-templates.md §2.1.1). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 accepts a requires-clause on a concept template parameter
//@ requires: clang>=22
//@ diag: 13:44 error "concept template parameter cannot have associated constraints"
template <class T> constexpr int sz = sizeof(T);
template <template <class T> requires true concept C> struct A1 {};
template <template <class T> requires true auto V> struct A2 {};
A2<sz> ok;
