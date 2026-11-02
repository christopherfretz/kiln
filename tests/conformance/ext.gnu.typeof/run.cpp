// GNU typeof (spec/sema.md §5.8) at run time: typeof is a keyword in the
// GNU dialects, a dependent typeof in a function template's signature
// mangles as clang's vendor type u6typeof, and the declared objects have
// the operand's type.
//@ expect: run
//@ args: -std=gnu++26
//@ stdout: 2.25 6 4 300 5 12 1
extern "C" int printf(const char*, ...);
template<class T> auto twice(T a) -> __typeof__(a + a);
template<class T> auto twice(T a) -> __typeof__(a + a) { return a + a; }
template<class T> __typeof(T) ident(__typeof(T) x) { return x; }
template<class T> struct Box { __typeof(T{} * 2) v; };
struct P { int x; double y; };
int main() {
  typeof(1.5) d = 2.25;
  __typeof(twice(short(3))) s = twice(short(3));
  Box<char> b{300};
  P p{1, 2.5};
  __typeof(p.y) y = p.y * 2;
  __typeof(int[3]) arr = {1, 2, 3};
  printf("%g %d %d %d %g %d %d\n", d, s, (int)sizeof(s), b.v, y, (int)sizeof(arr),
         ident<long>(7) == 7L);
}
