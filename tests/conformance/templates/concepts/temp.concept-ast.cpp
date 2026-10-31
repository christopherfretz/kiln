//@ expect: ast
//@ oracle: clang
// [temp.concept]: a concept definition and a concept-id evaluated in a
// variable initializer.
template <class T> concept Big = sizeof(T) > 4;
bool b = Big<double>;
bool c = Big<char>;
