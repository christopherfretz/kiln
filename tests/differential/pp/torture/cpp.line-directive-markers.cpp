//@ expect: pp
//@ pp-linemarkers
// The same #line forms, compared through the line markers of `-E` without
// `-P`: every token's presumed file and line must agree.
a
#line 100
b
c
#line 200 "renamed.cpp"
d
#define LN 300
#line LN
e


f
# 50 "gnu-style.cpp"
g
