//@ expect: pp
//@ oracle: none
//@ pp-compare: text
// The text rule sees spacing differences the token rule ignores.
#define F(x) f( x )
F(a+b);
