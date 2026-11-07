// [temp.over.link]/6.1 (P2841R7): a redeclaration of a class template
// whose template template parameter changes kind (type, concept, variable)
// does not have an equivalent template-head; kiln diagnoses it as clang
// does a type/non-type change. clang 22 accepts these silently, so this
// follows the wording (spec/sema-templates.md §6.8). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not compare template template parameter kinds across redeclarations
//@ requires: clang>=22
//@ diag: 17:33 error "template parameter has a different kind in template redeclaration"
//@ diag: 16:36 note "previous template declaration is here"
//@ diag: 19:36 error "template parameter has a different kind in template redeclaration"
//@ diag: 18:34 note "previous template declaration is here"
//@ diag: 21:42 error "template parameter has a different kind in template template parameter redeclaration"
//@ diag: 20:45 note "previous template template parameter is here"
template <template <class> concept C> struct S;
template <template <class> auto C> struct S {};
template <template <class> class C> struct R;
template <template <class> concept C> struct R {};
template <template <template <class> concept> class Q> struct N;
template <template <template <class> auto> class Q> struct N {};
