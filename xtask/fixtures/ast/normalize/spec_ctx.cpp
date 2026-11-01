namespace n {
template <class T, bool B = false, char C = 'a'>
struct X {
  struct N { T a; };
  template <class U> static int f(U uu) { return 0; }
};
struct S;
}
template <>
struct n::X<int>::N { long z; };
template <>
struct n::X<char> { struct N { int y; }; };
template struct n::X<unsigned, true, 'q'>;
template <class T>
struct n::X<T *, true> { struct N { T *p; }; };
int i = n::X<long>::f(1);
struct n::S { int sx; };
