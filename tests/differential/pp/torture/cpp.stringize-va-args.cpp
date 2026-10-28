//@ expect: pp
// `#__VA_ARGS__` stringizes the variable arguments including their commas,
// with the commas' spacing taken from the invocation.
#define S(...) #__VA_ARGS__
#define SX(first, ...) #first #__VA_ARGS__
S(a , b,c) S() S(  x  ) S(,) S( , , )
SX(1) SX(1,) SX(1, 2 ,3)
#define XS(...) S(__VA_ARGS__)
#define PAIR a,b
XS(PAIR) S(PAIR)
