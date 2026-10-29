// [temp.mem]/1, [temp.inst]/1: member class templates of a class template
// are redeclared in each specialization and instantiated on use; their
// defaults may refer to the enclosing template's parameters.
template <class T> struct A {
  template <class U, int N = sizeof(T), class V = U> struct B { T t; U u[N]; V v; };
  template <class U> struct C { using type = B<U>; };
  B<char> m;
};
A<int>::B<short> b;
A<long>::C<char>::type c;
static_assert(sizeof(A<int>::B<short>::u) == 2 * sizeof(int));
static_assert(sizeof(A<int>) == sizeof(A<int>::B<char>));
static_assert(sizeof(A<long>::C<char>::type::u) == sizeof(long));
static_assert(sizeof(A<char>::B<char, 3, short>) == 6);
