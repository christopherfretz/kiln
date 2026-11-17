// sides: pre post
// scope: defs
// A redeclaration of each function the header defines (the trailer side:
// after it), adding nothing that changes a definition: attributes not on
// the list may be added by a redeclaration ([dcl.attr.nodiscard],
// [dcl.attr.deprecated]).
namespace hp {
enum class Mode : short;
struct Base; struct Derived; struct Agg;
inline int plain(int a, int b);
[[nodiscard]] inline int isB(int e);
inline int lam(int y);
inline int generic(int z);
inline auto mk();
inline int use_mk();
inline int local_cls(int v);
inline int structured();
[[deprecated]] inline int statics();
template <class T> struct Box;
template <class T> T twice(T t);
template <class T> auto tsig(T t) -> decltype(t + 1) requires(sizeof(T) > 1);
template <class T> inline int tlam();
inline int callees();
[[noreturn]] inline void dies();
inline int attrs(int x);
inline const char *str();
inline long lit();
inline int func_name();
inline int news();
inline int tries();
inline int agg();
inline int dyncast(Base *b);
inline int sizes();
inline int loops(int n);
inline int stmt_expr();
inline int gotos(int n);
inline int builtins(int x);
constexpr int cexpr(int x);
consteval int ceval(int x);
inline int uses_ce();
inline auto deduced();
}
inline int use_all();
