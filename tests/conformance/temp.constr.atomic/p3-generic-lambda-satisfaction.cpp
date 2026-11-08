// [temp.constr.atomic]/3, [expr.prim.lambda.closure]/3: a generic lambda
// in a concept's constraint expression is substituted into during
// satisfaction checking like any other expression; its call operator
// template is then deduced and instantiated at the call. The lambda's
// invented parameter belongs to the lambda's own template head, one
// level deeper than the concept's ([expr.prim.lambda.general]/6), even
// though the concept's head is still open. Found with WP-821, fixed
// with WP-884. Verified against clang 19 (GCC 16.1 agrees).
//@ oracle: gcc
template <class T> concept K = [](auto x) { return sizeof(x) > 1; }(T{});
static_assert(K<long>);
static_assert(!K<char>);
template <class T> requires K<T> constexpr int f(T) { return 1; }
static_assert(f(1L) == 1);
int main() {}
