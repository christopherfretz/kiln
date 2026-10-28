//@ expect: pp
// The [lex.pptoken] exception: `<::` not followed by `:` or `>` lexes as
// `<` then `::`, not as the digraph `<:` then `:`. Re-lexing the output
// must observe the same split.
#define ID(x) x
template<class T> struct S;
S<::T> s1;
S<:::> s2;
S<::> s3;
a<::b;
ID(<::) ID(<:) ID(<) ID(::)
#define CAT(a, b) a ## b
CAT(<, :) :
