// [temp.deduct.type]/8: an array bound deduced through a reference to array.
template <class T, int N> constexpr int size(T (&)[N]) { return N; }
int a[5];
constexpr char s[] = "abc";
static_assert(size(a) == 5);
static_assert(size(s) == 4);
