export module aft;
export namespace aft {
template <class T> struct X;
using XI = X<int>;
template <class T> struct X { T v; int get() const { return (int)v + 1; } };
inline int use() { XI x{3}; return x.get(); }
}
