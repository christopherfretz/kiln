// _Generic (C11 6.5.1.1, a clang extension in C++): with only a `default`
// association the default is selected (WP-1131).
//@ expect: ast
//@ oracle: clang
int a = 1;
int z = _Generic(a, default: 3);
long w = _Generic(char, default: 4L);
