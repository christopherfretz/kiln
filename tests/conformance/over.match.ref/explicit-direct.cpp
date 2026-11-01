// [over.match.ref]/1.1, [dcl.init.ref]/5.3.2: a direct-initialization of a
// reference considers explicit conversion functions, for an rvalue, a
// const lvalue and a non-const lvalue reference, and for a class result.
//@ expect: run
struct Ex { explicit operator int() const { return 42; } };
struct ExR { int v = 7; explicit operator int&() { return v; } };
struct Y { int v; };
struct E2 { explicit operator Y() const { return {3}; } };
int main() {
  int&& r(Ex{});
  const int& c(Ex{});
  ExR x;
  int& l(static_cast<ExR&&>(x));
  const Y& y(E2{});
  int&& s = static_cast<int&&>(Ex{});
  return r == 42 && c == 42 && l == 7 && &l == &x.v && y.v == 3 && s == 42 ? 0 : 1;
}
