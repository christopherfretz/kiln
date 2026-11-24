// Function-name spans (spec/ast.md §24.4): unnamed entities are one
// synthesized entry over their spelling, naming the unnamed declaration;
// a tag named by a typedef for linkage prints, and names, the typedef.
// (An unnamed enum as a template argument prints the input's path:
// crates/kiln_parse/tests/fn_names.rs.) An unnamed class in the scope
// prints `(unnamed struct)` with clang 22 and `(anonymous struct)` with
// clang 19 (`-fclang-compat`), so each major has its own test.
//@ fn-names: unnamed-19.names
//@ std: c++20
//@ requires: clang<22
namespace ns {
struct { void m() { (void)__PRETTY_FUNCTION__; } } anon_obj;
typedef struct { void m() { (void)__PRETTY_FUNCTION__; } } Named;
typedef enum { e0 } NE;
template <class T> void ty(T) { (void)__PRETTY_FUNCTION__; }
template <class> struct U { static void m() { (void)__PRETTY_FUNCTION__; } };
}
void use() { ns::anon_obj.m(); ns::Named{}.m(); ns::ty(ns::e0); ns::U<int>::m(); }
