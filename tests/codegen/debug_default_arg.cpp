// Debug info (WP-941): a default argument is emitted with debug locations
// off (clang EmitCallArg's DisableDebugLocationUpdates), so the temporary
// it constructs takes the call's location, not the line of the parameter
// declaration (which, for a library template, is in a header and made
// gcov double-count the calling line).
//@ args: -g
struct A {
  A();
  ~A();
};
struct S {
  S(int p,
    const A & =
        A());
  ~S();
};
int parse(const S &s);
template <class T> struct U {
  U(int p,
    const T & =
        T());
};
int f(int = parse(3));
int main() {
  int ok = 0;
  ok += parse(1);
  U<A> u(2);
  ok += f();
  return ok;
}
