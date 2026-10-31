// [temp.func.order]: the more specialized template is called at run time.
//@ expect: run
template <class T> int f(T) { return 1; }
template <class T> int f(T*) { return 2; }
int main() { int x = 0; return f(&x) == 2 && f(x) == 1 ? 0 : 1; }
