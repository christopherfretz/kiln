// WP-1331, C95 6.4.6/3: digraphs came with amendment 1, so strict C89
// lexes `<:` `:>` `<%` `%>` `%:` `%:%:` as their separate characters
// and `%:define` is no directive (clang 19/22: no Digraphs in c89).
//@ std: c89
//@ expect: pp
//@ pp-compare: text
%:define D d
#define S(x) #x
#define T(x) S(x)
D <: :> <% %> %: %:%: T(<:) T(%:%:)
