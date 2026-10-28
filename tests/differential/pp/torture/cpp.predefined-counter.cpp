//@ expect: pp
// __COUNTER__ increments on every expansion, including in #if, but not when
// its name is stringized or pasted (it is not expanded then).
#define STR(x) #x
#define XSTR(x) STR(x)
#define CAT(a, b) a ## b
#define XCAT(a, b) CAT(a, b)
#define TWICE(x) x x
a __COUNTER__ __COUNTER__;
b TWICE(__COUNTER__);
c STR(__COUNTER__) CAT(v, __COUNTER__);
d XSTR(__COUNTER__) XCAT(v, __COUNTER__);
#if __COUNTER__ == 6
e six;
#endif
f __COUNTER__;
#define NEXT __COUNTER__
g NEXT NEXT;
