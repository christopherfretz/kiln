//@ expect: pp
// Comments in directives and macro definitions count as whitespace,
// including block comments spanning lines inside a directive and comments
// between a macro name and its parameter list.
#define A/**/1
#define B(x)/* c */x/* c */+x
#define C /* multi
  line */ 3
#/**/define D 4
#  /* c */ if /* c */ 1 // c
in_if;
#endif /* trailing */
A B(2) C D
#define E/**/(x) x
E(5)
#define STR(x) #x
STR(a/**/b) STR(a /* c */ b) STR(/* c */ c) STR(a // c
b)
