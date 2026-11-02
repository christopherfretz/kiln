// [lex.pptoken]/3.2: if the next three characters are `<::` and the
// subsequent character is neither `:` nor `>`, the `<` is a preprocessing
// token by itself (not the first character of the digraph `<:`). Otherwise
// `<:` is the digraph `[` (WP-309).
//@ expect: compile-ok
//@ strict-diags
struct X {};
template <class T> constexpr bool e = false;
template <> constexpr bool e<::X> = true;
template <class T> struct W { static constexpr int v = 1; };
static_assert(e<::X> && !e<int>);
static_assert(W<::X>::v == 1);
constexpr int k = 1;
constexpr int a<::> = {4, 5};   // `<::>`: `<:` `:>`, i.e. `[]`
static_assert(a<:::k:> == 5);   // `<:::`: `<:` `::`, i.e. `a[::k]`
static_assert(sizeof(a) == 2 * sizeof(int));
