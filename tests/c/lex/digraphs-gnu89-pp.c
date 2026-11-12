// WP-1331, C95 6.4.6/3: -std=gnu89 lexes digraphs (clang 19/22), so
// `%:define` is a directive and `%:%:` pastes.
//@ std: c89
//@ args: -std=gnu89
//@ expect: pp
//@ pp-compare: text
%:define D d
%:define CAT(a, b) a %:%: b
#define S(x) #x
#define T(x) S(x)
D CAT(x, y) <: :> <% %> T(<:) T(%:%:)
