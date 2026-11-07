// P2308R1 ([temp.arg.nontype]/1): a braced-init-list template argument is
// copy-list-initialization, so narrowing is ill-formed; a placeholder `auto`
// parameter cannot be deduced from one (kiln keeps clang's wording for a
// missing <initializer_list>, also when the header is included).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:4 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 14:4 note "insert an explicit cast to silence this issue"
//@ diag: 14:3 warning "braces around scalar initializer"
//@ diag: 14:4 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 15:3 error "cannot deduce type of initializer list because std::initializer_list was not found; include <initializer_list>"
template <int I> struct G {};
template <auto V> struct X {};
G<{1.5}> g;
X<{1}> x;
