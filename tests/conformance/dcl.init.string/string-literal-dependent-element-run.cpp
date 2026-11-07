// [dcl.init.string]/1, [temp.dep.type]/9: `T a[N] = "abc";` with a
// dependent element type is initialized at instantiation, where a
// char/signed char/unsigned char array takes the literal (an unbounded
// array its bound, [dcl.array]/7). WP-681.
//@ expect: run
//@ strict-diags
template <class T> int f() {
  T a[10] = "abc";
  T b[] = "xy";
  return sizeof(b) + a[2] + sizeof(a);
}
template <class T> struct W { T m[4] = "pq"; };
int main() {
  return f<char>() + f<signed char>() + f<unsigned char>() + W<char>{}.m[1]
       - 3 * (3 + 'c' + 10) - 'q';
}
