//@ expect: ast
//@ oracle: clang
// [temp.param]/12-14: a default template argument may refer to earlier
// parameters; omitted arguments are substituted in order.
template <class T, class U = T *, int N = sizeof(T)> struct Trip {
  U p;
  char pad[N];
};
int a = sizeof(Trip<int>);
int b = sizeof(Trip<char, long>);
