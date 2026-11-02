// [temp.deduct.call]/1, [temp.deduct.funcaddr]/1, [temp.func.order]/1,
// [temp.friend]/1, [temp.arg.general]/9: a function template first
// declared as a friend inside a class template (one or two levels deep,
// before or after the enclosing template is instantiated) deduces,
// completes its arguments from defaults, takes its address and orders
// against an overload with the namespace scope declaration's own
// parameters: a friend in a templated class redeclares nothing until it is
// instantiated, and then with the parameters substituted to depth 0
// (WP-299 audit; the class template counterpart is
// temp.deduct.type/default-arg-friend-first-declaration.cpp). Verified
// against clang 19.
//@ expect: run
//@ exit: 18
template <class T> struct W {
  template <class U, class V> friend int f(U, V*);
  template <class U, class V> friend int f(U, V);
  template <class U, class V> friend int d(U);
};
template <class T> struct O {
  template <class X> struct I {
    template <class U, class V> friend int e(U);
  };
};
W<int> w;
O<int>::I<char> oi;
template <class U, class V = char> int f(U, V*) { return 1; }
template <class U, class V> int f(U, V) { return 2; }
template <class U, class V = U[2]> int d(U);
template <class U, class V> int d(U) { return sizeof(V); }
template <class U, class V = U[3]> int e(U);
template <class U, class V> int e(U) { return sizeof(V); }

int main() {
  int i = 0;
  int (*p)(int, int*) = &f;   // 1
  int (*q)(int, char*) = &f<int>; // 1
  int (*r)(char) = &d;       // 2
  return f(1, &i) + f(1, 2) + p(1, &i) + q(1, nullptr) + d('c') + r('c') + e('c') * 3 + e(short(1)) - 6;
}
