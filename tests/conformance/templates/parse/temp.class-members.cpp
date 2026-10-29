//@ expect: ast
//@ oracle: clang
// [temp.class]: class template members using the parameters
template <class T, int N> struct Array {
  T data[N];
  static const int size = N;
  T &at(int i) { return data[i]; }
  int length() const { return N; }
};
