// [temp.arg.nontype]: reference and decayed-array arguments at run time
// (and their manglings: L_Z2giE, Xadso...E).
//@ expect: run
//@ stdout: 5 4 7 2 4
extern "C" int printf(const char*, ...);
struct A { int x; };
A ga{5};
int gi = 3;
int arr[3] = {1, 2, 3};
int seven() { return 7; }
template <A& a> int ax() { return a.x; }
template <int& r> void bump() { ++r; }
template <int (&f)()> int call() { return f(); }
template <int* p> int at1() { return p[1]; }
template <const int& r> struct H { static int get() { return r; } };
int main() {
  bump<gi>();
  printf("%d %d %d %d %d\n", ax<ga>(), gi, call<seven>(), at1<arr>(), H<gi>::get());
}
