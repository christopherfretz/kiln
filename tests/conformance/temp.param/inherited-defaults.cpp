// [temp.param]/12: the default template arguments available are those of
// all the declarations of the template, whichever declaration has them;
// [temp.over.link]/1: a function template declared before a class
// template's definition is redeclared by its definition after it.
template <class T, class U = int> struct B;
template <class T, class U> struct B { U x; };
template <class T> struct C;
template <class T = long> struct C { T x; };
B<char> b;
C<> c;
static_assert(sizeof(b.x) == sizeof(int));
static_assert(sizeof(c.x) == sizeof(long));
template <class T> int g(B<T>);
template <class T> int g(B<T>) { return 1; }
int use = g(b);
