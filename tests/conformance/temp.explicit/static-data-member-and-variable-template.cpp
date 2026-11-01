// [temp.explicit]/3,12,14: explicit instantiation of static data members and variable template
// specializations; a definition instantiates the initializer, a declaration provides none
//@ expect: run
//@ strict-diags
template <class T> struct S { static const int n = 4; static T m; static int k; };
template <class T> T S<T>::m = T(7);
template <class T> int S<T>::k = sizeof(T);
template <class T> T v = T(3);
extern template const int S<char>::n;
extern template long S<long>::m;
template long S<long>::m;
template int S<short>::k;
template int v<int>;
extern template double v<double>;
template <class T> struct U { static int u; };
template <class T> int U<T>::u = 5;
template int U<int>::u; // instantiated and emitted though unused here
template struct U<char>;
int main() {
  return S<long>::m == 7 && v<int> == 3 && S<short>::k == 2 && S<char>::n == 4 && U<char>::u == 5
             ? 0
             : 1;
}
