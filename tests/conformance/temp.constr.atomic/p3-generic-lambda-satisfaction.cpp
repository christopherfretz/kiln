// [temp.constr.atomic]/3, [expr.prim.lambda.closure]/3: a generic lambda
// in a concept's constraint expression is substituted into during
// satisfaction checking like any other expression; its call operator
// template is then deduced and instantiated at the call. Found with
// WP-821. Verified against clang 19 (GCC 16.1 agrees).
//@ xfail(kiln): WP-821 follow-up, substituting a generic lambda during satisfaction fails in lambda_declarator (crates/kiln_sema/src/templ/subst/lambda.rs) and the atom reads as ill-formed
template <class T> concept K = [](auto x) { return sizeof(x) > 1; }(T{});
static_assert(K<long>);
static_assert(!K<char>);
template <class T> requires K<T> constexpr int f(T) { return 1; }
static_assert(f(1L) == 1);
int main() {}
