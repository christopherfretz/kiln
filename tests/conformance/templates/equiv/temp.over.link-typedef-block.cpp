// [dcl.typedef]/4, [temp.over.link]/5: a block-scope typedef in a
// template redeclared with an equivalent dependent type.
template <class T> int f() {
  typedef int A[sizeof(T) * 2];
  typedef int A[sizeof(T) * 2];
  return sizeof(A);
}
int r = f<int>();
