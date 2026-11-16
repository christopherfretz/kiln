// [expr.reflect]: `^^v` names `v` as any other id-expression does, so a
// variable, parameter or structured binding whose only use is its
// reflection is referenced: no -Wunused-variable, -Wunused-parameter or
// -Wunused-but-set-variable for it, whether the reflection is spliced as a
// value, only queried, only used for its type, or passed as a template
// argument (GCC 16.1 warns for none of them). A file-scope variable with
// internal linkage that is only reflected gets no "is not needed and will
// not be emitted" either. An entity that is never named still warns.
// Reported by a user: a constexpr local passed as
// `X<^^expression>` to a template that reads only its type.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain} -Wall -Wextra
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 45:19 warning "unused variable 'really_unused'"
//@ diag: 46:9 warning "variable 'really_set' set but not used"
//@ diag: 53:7 warning "unused variable 'unused_in_template'"
//@ diag: 56:31 note "in instantiation of function template specialization 'g<int>' requested here"
#include <meta>
using namespace std::meta;
int f(int p1, int p2);
template <info R> struct TypeOnly { using type = [:type_of(R):]; };
template <info R> struct Queried { static constexpr bool var = is_variable(R); };
static int file_static = 1;
namespace { int in_unnamed = 2; }
constexpr bool file_scope = is_variable(^^file_static) && is_variable(^^in_unnamed);
struct P { int a; int b; };
int f(int p1, int p2) {
    constexpr bool x1 = is_function_parameter(^^p1);
    constexpr auto expression = 3 * 7;
    TypeOnly<^^expression>::type z = 0;
    constexpr int queried = 1;
    bool x2 = Queried<^^queried>::var;
    int set_only;
    set_only = 1;
    constexpr bool x3 = is_variable(^^set_only);
    P s{1, 2};
    auto [b1, b2] = s;
    constexpr bool x4 = is_structured_binding(^^b1);
    static int local_static = 4;
    constexpr bool x5 = is_variable(^^local_static);
    int sized = 5;
    unsigned long x6 = sizeof(^^sized);
    constexpr int really_unused = 9;
    int really_set;
    really_set = 1;
    return x1 + z + x2 + x3 + x4 + x5 + (int)x6;
}
template <class T> int g() {
    T reflected = 0;
    constexpr bool x = is_variable(^^reflected);
    T unused_in_template = 0;
    return x;
}
int main() { return f(1, 2) + g<int>() + file_scope; }
