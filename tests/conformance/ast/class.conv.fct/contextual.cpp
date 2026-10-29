//@ expect: ast
//@ oracle: clang
// [conv.general]/4, [class.conv.fct]/2: contextual conversions to bool
// by explicit and implicit conversion functions; a returned prvalue
// object materialized for the call.
struct E { explicit operator bool() const; };
struct I { operator int() const; };
struct Pp { operator int*(); };
bool g(E e, I i, Pp p) {
  if (e) {}
  while (i) {}
  bool a = !e;
  bool b = e && i;
  bool c = e || p;
  int d = e ? 1 : 2;
  for (; p;) {}
  do {} while (e);
  return static_cast<bool>(e);
}
long h(I i) { return i; }
I mk();
int k() { return mk(); }
