// GCC "Specifying Attributes of Types", vector_size: an attribute in the
// decl-specifiers applies to the type they name, the element type, before
// their cv-qualifiers, so `__attribute__((vector_size(16))) double const *p`
// points to a const vector of two doubles, not to a vector of const double
// (clang `processTypeAttrs` on the decl-spec type, then `ConvertDeclSpecToType`'s
// qualifiers). The shape is the one reduced from a downstream build
// triage (the originating header was not located).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:5 error "cannot initialize a variable of type 'int' with an lvalue of type '__attribute__((__vector_size__(2 * sizeof(double)))) double const *'"
//@ diag: 22:5 error "cannot initialize a variable of type 'int' with an rvalue of type '__attribute__((__vector_size__(2 * sizeof(int)))) int const volatile *'"

__attribute__((vector_size(16))) double const *p;
__attribute__((vector_size(16))) const double *q;
typedef double v2 __attribute__((vector_size(16)));
const v2 *r = p;                                        // OK: same type
static_assert(__is_same(decltype(p), const v2 *));
static_assert(__is_same(decltype(q), const v2 *));
static_assert(sizeof(*p) == 16);
int x = p;
volatile __attribute__((vector_size(8))) int const vc = {1, 2};
int y = &vc;
