// sides: pre post
// scope: any
// Unrelated overload sets with the definitions' names in other namespaces
// and at global scope.
namespace kc_ov {
inline int plain(double, int = 1) { return 0; }
template <class T> T twice(T t, int) { return t; }
template <class T> struct Box { T u; };
inline int isB(long) { return 1; }
enum Color { Red = 9 };
}
inline int twice(char c) { return c; }
inline int swap(double, double) { return 0; }
