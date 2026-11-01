// The traits that need overload resolution's unevaluated init_check /
// conv_check (spec/sema-templates.md §9.1, owner WP-068a) report
// sema_unimplemented_template_feature until then; so does a trait whose
// class property kiln does not compute yet. __has_builtin is 0 for them
// (has-builtin-staged-off.cpp), so libc++ does not use them.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 19 implements these traits
//@ diag: 13:10 error "not yet implemented: type trait '__is_trivially_relocatable'"
//@ diag: 14:10 error "not yet implemented: type trait '__is_trivially_equality_comparable'"
//@ diag: 15:10 error "not yet implemented: type trait '__is_layout_compatible'"
struct S { int x; };
bool e = __is_trivially_relocatable(S);
bool f = __is_trivially_equality_comparable(S);
bool g = __is_layout_compatible(S, S);
