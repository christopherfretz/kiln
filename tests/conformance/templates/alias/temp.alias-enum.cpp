//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: an alias template naming an enumeration type.
enum Color { Red, Green };
template <class T> using Same = T;
Same<Color> c = Green;
