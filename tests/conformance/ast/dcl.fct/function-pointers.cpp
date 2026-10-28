//@ expect: ast
int add(int a, int b) { return a + b; }
int (*op)(int, int) = add;
int (*ops[2])(int, int) = {add, &add};
int run(int (*f)(int, int)) { return f(1, 2) + (*f)(3, 4) + op(5, 6) + ops[1](7, 8); }
