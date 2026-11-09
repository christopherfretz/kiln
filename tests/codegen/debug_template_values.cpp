// Debug info (WP-814): non-integral template value arguments; a null
// member function pointer is `i8 0` like any null pointer (WP-910).
//@ args: -g
struct S { int a; long b; };
struct M { int x, y; int f(); };
int g;
int arr[4];
int fn(int);
template <auto V> struct T { int k = 0; };
T<&g> t1;
T<fn> t2;
T<S{3, 4}> t3;
T<&M::y> t4;
T<&M::f> t5;
T<(int M::*)nullptr> t6;
T<(int (M::*)())nullptr> t7;
T<&arr[2]> t8;
int use() { return t1.k + t2.k + t3.k + t4.k + t5.k + t6.k + t7.k + t8.k; }
