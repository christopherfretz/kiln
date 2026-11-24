// Function-name spans (spec/ast.md §24.2): a string reached through a
// macro is recorded as any other; the record belongs to the function the
// expansion is in.
//@ fn-names: macros.names
//@ std: c++20
#define FN __PRETTY_FUNCTION__
#define LOG(x) ((void)(x), (void)FN)
namespace ns {
template <class T> void log(T t) { LOG(t); }
void plain() { LOG(0); }
}
void use() { ns::log(1.0); ns::plain(); }
