// [temp.constr.normal]/1.5.3 (P2841R7): a fold expression in a constraint
// that contains a concept template parameter pack shall not contain an
// unexpanded pack of another kind ([temp.constr.normal] Example 5); kiln
// diagnoses it where the constraint is formed. clang 22 accepts Example 5,
// so this follows the wording (spec/sema-templates.md §8.2). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not diagnose a fold mixing concept and type packs
//@ requires: clang>=22
//@ diag: 13:36 error "fold expression in a constraint contains a concept template parameter pack and a template parameter pack of another kind"
template <typename T, template <typename> concept> struct wrapper {};
template <typename... T, template <typename> concept... CTs>
int f(wrapper<T, CTs>...) requires (CTs<T> && ...);
template <typename T, template <typename> concept... CTs>
concept bad = (CTs<T> || ...) && (CTs<T*> && ...);
template <typename T, template <typename> concept... CTs>
int g() requires (CTs<T> && ...) && (CTs<T*> || ...);
template <typename... T> int h() requires (__is_same(T, int) && ...);
