//@ expect: pp
//@ pp-linemarkers
// Presumed locations of tokens produced by multi-line macro invocations
// and of the lines that follow them, in line-marker mode.
#define F(a, b) a + b
#define OBJ one two
F(1,
  2) after_f;
next_line OBJ;
F(
  OBJ
  ,
  3
)
F(x, y) /* comment
spanning lines */ tail;
last;
