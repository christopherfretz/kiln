//@ expect: ast
//@ oracle: clang
// [temp.dep.candidate]/1, [basic.lookup.argdep]: unqualified dependent calls
template <class T> void caller(T t) { helper(t); }
void known(int);
template <class T> void caller2(T t) { known(t); }
