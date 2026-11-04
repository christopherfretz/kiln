// [dcl.array]/3, [class.static.data]/3, [temp.inst]/4: a static data member
// of a class template declared as an array of unknown bound takes its bound
// from the out-of-class definition. Instantiating that definition gives it
// the definition's (here dependent) type, not the in-class declaration's
// `T[]`, so it is not "definition of variable with array type needs an
// explicit size". The second shape is libstdc++'s copy-on-write
// `basic_string::_Rep::_S_empty_rep_storage` (bits/cow_string.h, the
// pre-C++11 ABI that devtoolset/gcc-toolset GCCs force): a nested class's
// member whose bound is computed from names of the enclosing class.
//@ expect: run
//@ strict-diags
//@ stdout: 0 3 4
extern "C" int printf(const char*, ...);
template <class T> struct S { static unsigned long st[]; };
template <class T> unsigned long S<T>::st[(sizeof(T) + 7) / 8];
template <class C> struct Str {
  typedef unsigned long size_type;
  struct Rep_base {
    size_type len, cap;
    int refcount;
  };
  struct Rep : Rep_base { static size_type empty_storage[]; };
};
template <class C>
typename Str<C>::size_type Str<C>::Rep::empty_storage[
    (sizeof(Rep_base) + sizeof(C) + sizeof(size_type) - 1) / sizeof(size_type)];
int main() {
  S<char[24]>::st[2] = 3;
  Str<char>::Rep::empty_storage[3] = 4;
  printf("%lu %lu %lu\n", S<int>::st[0], S<char[24]>::st[2], Str<char>::Rep::empty_storage[3]);
}
