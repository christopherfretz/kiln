// [temp.inst]/5,7: a static data member of an implicitly instantiated class template specialization
// is defined when it is used, whether its definition is in the class (an inline constexpr member)
// or out of line; libc++'s numeric_limits<T>::digits taken by address
//@ expect: run
//@ strict-diags
template <class T> struct S {
  static int k;
  static const int c;
  static constexpr int d = 5;
  static const int* addr() { return &d; }
};
template <class T> int S<T>::k = sizeof(T);
template <class T> const int S<T>::c = 3;
template <class T> struct L { static constexpr int digits = sizeof(T) * 8; int n = digits; };
int main() {
  const int* p = &L<short>::digits;
  L<char> l;
  return (S<long>::k == sizeof(long) && S<int>::c == 3 && *&S<char>::d == 5 && *S<int>::addr() == 5
          && *p == 16 && l.n == 8) ? 0 : 1;
}
