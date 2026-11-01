// [temp.names]/1, [expr.prim.id]/4, [expr.prim.req]/2: a concept-id or
// requires-expression evaluated at run time is a bool prvalue constant.
//@ expect: run
//@ exit: 0
template<class T> concept K = sizeof(T) == 4;
template<class T> concept Big = sizeof(T) > 4;
template<class T> bool test() { return K<T>; }
template<class T> bool has_x() { return requires { T::x; }; }
struct WithX { static const int x = 1; };
bool f() { return K<int>; }
int main() {
  bool b = K<int>;
  bool c = Big<char>;
  bool d = requires(int a) { a + 1; };
  int r = b + 2 * c + 4 * f() + 8 * test<int>() + 16 * test<char>() + 32 * !Big<char> + (Big<long> ? 64 : 0);
  r += 128 * d + 256 * has_x<WithX>() + 512 * has_x<int>();
  return r - (1 + 4 + 8 + 32 + 64 + 128 + 256);
}
