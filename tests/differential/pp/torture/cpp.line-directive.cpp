//@ expect: pp
// #line sets the presumed line (and file) of the next line; its operands
// are macro replaced.
a __LINE__ __FILE__
#line 100
b __LINE__
c __LINE__
#line 200 "renamed.cpp"
d __LINE__ __FILE__
#define LN 300
#define FN "macro-named.cpp"
#line LN FN
e __LINE__ __FILE__
#line 7 "with spaces and \"quotes\".cpp"
f __LINE__ __FILE__
# 50 "gnu-style.cpp"
g __LINE__ __FILE__
