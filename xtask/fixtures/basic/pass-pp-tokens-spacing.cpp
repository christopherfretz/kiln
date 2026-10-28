//@ expect: pp
//@ oracle: none
// Token-wise comparison ignores spacing between tokens, even where clang
// would print a space and the golden does not.
#define F(x) f( x )
F(a+b);
int  x=1 ;
