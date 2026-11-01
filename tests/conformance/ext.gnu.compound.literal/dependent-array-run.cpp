// Dependent array compound literals ([GNU]; [temp.dep.type]): the
// element type, the bound, and a bound deduced from a value-dependent
// list are all resolved per instantiation.
//@ expect: run
//@ exit: 0
template<class T> T f() { return ((T[]){1, 2, 3})[1]; }
template<int N> int g() { return ((int[N]){1, 2})[1]; }
template<int N> int h() { return sizeof((int[]){N, N, N}) / sizeof(int); }
template<class T, int N> T s() { T r = 0; for (T e : (T[N]){4, 5}) r += e; return r; }
int main() { return f<int>() + g<3>() + h<7>() + s<long, 3>() - 16; }
