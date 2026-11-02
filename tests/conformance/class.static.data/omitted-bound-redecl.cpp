// [dcl.array]/8, [basic.link]/11, [class.static.data]/2: the out-of-class
// definition of a static data member array may omit the bound declared in
// the class; it redeclares that member (one entity for lookup) and takes the
// declared bound, so an initializer shorter than the bound value-initializes
// the rest. The same holds for a namespace-scope variable redeclared without
// its bound, and an omitted in-class bound is completed by the definition.
//@ expect: run
//@ strict-diags
//@ stdout: 16 16 12 2 0 16 2
extern "C" int printf(const char*, ...);
extern int a[4];
int a[] = {1, 2};
struct S {
  enum { os = 4 };
  static int b[os];
  static int c[];
};
int S::b[] = {1, 2};
int S::c[] = {1, 2, 3};
template <class T> struct U { static T d[4]; };
template <class T> T U<T>::d[] = {1, 2};
int main() {
  const int* p = S::b;
  printf("%zu %zu %zu %d %d %zu %d\n", sizeof(a), sizeof(S::b), sizeof(S::c), p[1], S::b[3],
         sizeof(U<int>::d), U<int>::d[1]);
}
