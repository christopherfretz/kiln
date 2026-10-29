// [dcl.init.aggr]: aggregate members without an initializer are
// value-initialized, and braces may be elided for nested aggregates.
//@ expect: run
//@ stdout: 1 2 0 0 | 5 0 0 | 7 8 9 10
extern "C" int printf(const char*, ...);

struct Inner {
  int a, b;
};
struct Outer {
  Inner in;
  int c, d;
};

int main() {
  Outer o = {{1, 2}};
  int arr[3] = {5};
  Outer e = {7, 8, 9, 10};
  printf("%d %d %d %d | %d %d %d | %d %d %d %d\n", o.in.a, o.in.b, o.c, o.d, arr[0], arr[1],
         arr[2], e.in.a, e.in.b, e.c, e.d);
}
