//@ expect: ast
int a = 1;
int b(2);
int c{3};
int d = {4};
double e{};
int f = a + b + c + d + e;
struct S { int x; double y; };
S s1 = {1, 2.0};
S s2{3};
