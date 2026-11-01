// [temp.inst]/7, [temp.explicit]/12: a variable template specialization or a static
// data member of a class template specialization odr-used with no definition
// anywhere in the TU is -Wundefined-var-template at its point of instantiation
// (clang InstantiateVariableDefinition at the end of the TU), with a note at the
// pattern's latest declaration and the explicit-instantiation-declaration hint.
// Not reported: a definition later in the TU, a variable usable in constant
// expressions, an unevaluated reference, an explicit specialization or explicit
// instantiation declaration (also of the enclosing class), a pattern in a system
// header. A variable template specialization is reported once per odr-use.
//@ args: -isystem %{test_dir}/Inputs
//@ strict-diags
//@ diag: 40:20 warning "instantiation of variable 'w<int>' required here, but no definition is available"
//@ diag: 36:29 note "forward declaration of template entity is here"
//@ diag: 40:20 note "add an explicit instantiation declaration to suppress this warning if 'w<int>' is explicitly instantiated in another translation unit"
//@ diag: 40:46 warning "instantiation of variable 'S<int>::m' required here, but no definition is available"
//@ diag: 38:42 note "forward declaration of template entity is here"
//@ diag: 40:46 note "add an explicit instantiation declaration to suppress this warning if 'S<int>::m' is explicitly instantiated in another translation unit"
//@ diag: 40:20 warning "instantiation of variable 'w<int>' required here, but no definition is available"
//@ diag: 36:29 note "forward declaration of template entity is here"
//@ diag: 40:20 note "add an explicit instantiation declaration to suppress this warning if 'w<int>' is explicitly instantiated in another translation unit"
//@ diag: 43:29 warning "instantiation of variable 'U<int>::m' required here, but no definition is available"
//@ diag: 41:42 note "forward declaration of template entity is here"
//@ diag: 43:29 note "add an explicit instantiation declaration to suppress this warning if 'U<int>::m' is explicitly instantiated in another translation unit"
//@ diag: 43:41 warning "instantiation of variable 'S<int>::mt<int>' required here, but no definition is available"
//@ diag: 38:95 note "forward declaration of template entity is here"
//@ diag: 43:41 note "add an explicit instantiation declaration to suppress this warning if 'S<int>::mt<int>' is explicitly instantiated in another translation unit"
//@ diag: 45:32 warning "instantiation of variable 'N::S<int>::m' required here, but no definition is available"
//@ diag: 44:56 note "forward declaration of template entity is here"
//@ diag: 45:32 note "add an explicit instantiation declaration to suppress this warning if 'N::S<int>::m' is explicitly instantiated in another translation unit"
//@ diag: 45:39 warning "instantiation of variable 'N::vv<N::S<int>>' required here, but no definition is available"
//@ diag: 44:92 note "forward declaration of template entity is here"
//@ diag: 45:39 note "add an explicit instantiation declaration to suppress this warning if 'N::vv<N::S<int>>' is explicitly instantiated in another translation unit"
#include <undefined-var-template.h>
template <class T> extern T v;
template <class T> extern T w;
template <class T> extern T w;
template <> short w<short> = 1;
template <class T> struct S { static int m; static const int c; template <class U> static int mt; };
extern template struct S<char>;
int use() { return w<int> + v<int> + S<int>::m + S<char>::m + w<int> + S<long>::c; }
template <class T> struct U { static int m; };
template struct U<int>;
int use2() { return U<int>::m + S<int>::mt<int>; }
namespace N { template <class T> struct S { static int m; }; template <class T> extern int vv; }
int use3() { return N::S<int>::m + N::vv<N::S<int>>; }
template <class T> extern int x;
template <> int x<int>;
template <class T> extern int y;
extern template int y<int>;
template <class T> extern const T cv;
int use4() { return x<int> + y<int> + cv<int> + sizeof(v<long>) + sizeof(S<long>::m); }
template <class T> int tf() { return v<T> + w<T>; }
int use5() { return tf<short>() + sys_v<int> + SysS<int>::m; }
template <class T> T v = T();
