// [dcl.attr.grammar]/6: an attribute-token clang does not know in that
// spelling is ignored with a warning (WP-667; spec/sema.md §4.5). The
// `Clang<"x">` lifetime attribute has no `gnu::` or unscoped `[[x]]`
// form, and the `GCC<"x">` sanitizer attributes have no `clang::` form.
// (`[[gnu::no_sanitize]]` depends on the release:
// gnu-no-sanitize-clang19.cpp, gnu-no-sanitize-clang22.cpp.)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 12:3 warning "no_sanitize_address' ignored"
//@ diag: 13:17 warning "unknown attribute 'lifetimebound' ignored"
//@ diag: 14:17 warning "lifetimebound' ignored"
[[clang::no_sanitize_address]] int f2();
void g(int &a [[lifetimebound]]);
void h(int &a [[gnu::lifetimebound]]);
