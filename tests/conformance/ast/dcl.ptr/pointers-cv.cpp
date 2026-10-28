//@ expect: ast
int x;
int *p = &x;
const int *pc = p;
int *const cp = &x;
int **pp = &p;
const volatile int *pcv = nullptr;
int deref() { return **pp + *pc + *cp; }
