//@ expect: pp
//@ pp-compare: text
// A printer test, compared as text: the -E output keeps a single space
// where the source had whitespace before a token, none where it had none,
// and a macro expansion's first token takes the invocation's spacing. Only
// spacing within a line is pinned (runs collapse, lines are trimmed).
#define EMPTY
#define OBJ   a   +   b
#define F(x)  ( x )
f(a,b)  g( a , b )	h (a)
OBJ;OBJ ; F(1)F( 2 )
x EMPTY y EMPTY;
