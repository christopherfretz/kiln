//@ expect: ast
int a[3];
int b[] = {1, 2, 3, 4};
char s[] = "hi";
int m[2][3] = {{1, 2, 3}, {4}};
int total() { return a[0] + b[3] + s[1] + m[1][2] + sizeof(b) / sizeof(b[0]); }
