// [temp.over.link]/5, [dcl.array]: dependent array bounds compare by
// equivalence.
template <class T> int h(int (&)[sizeof(T)]);
template <class U> int h(int (&a)[sizeof(U)]) { return sizeof(a); }
int arr[sizeof(int)];
int r = h<int>(arr);
