//@ expect: pp
// [cpp.predefined]/1 __LINE__: a __LINE__ produced by a macro's replacement
// list in a multi-line invocation takes the line of the invocation's closing
// `)` (clang's presumed expansion location); a __LINE__ written as an
// argument keeps its own line; nesting follows the outermost `)`.
#define F(x) x
#define G(x, y) x y __LINE__
#define H() __LINE__
#define ID(x) x
int a = F(
  __LINE__
);
int b[] = { G(1,
  +
  2) };
int c = H(
);
int d = ID(F(
  __LINE__) +
  H()
);
int e = ID(
  ID(
    __LINE__
  )
);
int f = __LINE__;
