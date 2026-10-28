//@ expect: pp
// Digraphs as directive introducers, as the # and ## operators in macro
// bodies, and under stringization (which keeps the digraph spelling).
%:define STR(x) %:x
%:define CAT(a, b) a %:%: b
%:define XSTR(x) STR(x)
STR(<: :> <% %> %: %:%:) STR([ ] { } # ##)
CAT(<, :) CAT(%, >) CAT(%:, %:)
XSTR(CAT(<, %))
int a<:2:> = <% 1, 2 %>;
%:if 1
digraph_if;
%:endif
