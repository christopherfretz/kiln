//@ expect: ast
//@ ast-filter: C1
// [temp.param]/14, [temp.arg.general]/9, [dcl.array], [dcl.fct]/5:
// clang's -ast-dump-filter names a member of a class template
// specialization without the trailing arguments equal to their defaults.
// A default with an array or function declarator is spelled after
// substitution as clang nests declarators (`long (*)[4]`, `void (long)`,
// `char *const[3]`), with parameter types adjusted (`T[2]` → `T *`).
// clang compares a written argument with such a default structurally and
// never finds it equal (`isSubstitutedDefaultArgument`), so a written
// argument always prints and only an omitted one is dropped.
template <class T, class A = T[3], class B = T[2][3], class C = T (*)[4],
          class D = T (*)(int), class E = void(T), class F = void (*)(T *, T[2]),
          class G = const T[3], class H = T (&)[2], class I = int (*)(const T, ...)>
struct X { struct C1 { int x; }; };
template <> struct X<int>::C1 { int a; };
template <> struct X<int, long[3]>::C1 { int b; };
template <> struct X<char *>::C1 { int c; };
template <> struct X<long, long[3], long[2][3], long (*)[4], long (*)(int), void(long),
                     void (*)(long *, long *)>::C1 { int d; };
template <> struct X<short, short[3], short[2][3], short (*)[4], short (*)(int),
                     void(short), void (*)(short *, short *), const short[3],
                     short (&)[2], int (*)(short, ...)>::C1 { int e; };
template <class T, class U, class P = U (*)(T (*)[2], void (*)(U))>
struct Y { struct C1 { int y; }; };
template <> struct Y<int, long>::C1 { int f; };
template <> struct Y<short, long, long (*)(short (*)[2], void (*)(long))>::C1 { int g; };
template <> struct Y<int, long, long (*)(int *, void (*)(long))>::C1 { int h; };
