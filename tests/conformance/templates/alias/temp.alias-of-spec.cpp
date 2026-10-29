//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: an alias template naming a class template specialization.
template <class T, class U> struct Pair { T a; U b; };
template <class T> using Twin = Pair<T, T>;
Twin<int> *tp;
int sz = sizeof(Twin<short>);
