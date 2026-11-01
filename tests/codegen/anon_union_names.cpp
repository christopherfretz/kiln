// [class.union.anon]: the LLVM types of anonymous unions and structs are
// named `%union.anon` / `%struct.anon` with clang's numbering, whatever
// class, namespace or template encloses them.
struct U { union { int a; float b; }; int z; };
struct V { union { long a; double b; }; struct { int p, q; }; };
union W { struct { char c; int d; }; int e; };
namespace ns { struct X { union { short s; }; }; }
int use() { U u{}; V v{}; W w{}; ns::X x{}; return u.a + v.p + w.d + x.s; }
template<class T> struct TT { union { T t; int i; }; };
TT<char> tt;
