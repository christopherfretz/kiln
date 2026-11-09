// [temp.inst]/7, [dcl.array]/3: a variable template specialization whose
// pattern has an array of unknown bound is emitted with the bound its
// initializer gives: a list, a pack expansion, a string literal, a
// static data member template, a constexpr one, and an explicit
// specialization with a bound of its own (WP-907).
template <class T> T arr[] = {1, 2, 3};
template <class T> constexpr T carr[] = {4, 5};
template <> long arr<long>[] = {7};
template <class... T> int packed[] = {sizeof(T)...};
template <class T> char str[] = "hello";
struct S { template <class T> static T m[]; };
template <class T> T S::m[] = {1, 2, 3, 4};
unsigned long f() {
  return sizeof(arr<int>) + sizeof(arr<long>) + sizeof(packed<char, int>) + sizeof(str<int>) +
         sizeof(S::m<int>);
}
int *g() { return &arr<int>[1]; }
const int *h() { return carr<int>; }
char *k() { return str<int>; }
int *l() { return S::m<int>; }
int *m() { return packed<char, int>; }
long *n() { return arr<long>; }
