// [except.pre]/1, [temp.variadic]/5: an exception-declaration declares
// no pack, so a `...` in it is ill-formed ("only function and template
// parameters can be parameter packs", at the ellipsis) and its type then
// holds an unexpanded pack (at the declarator).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:18 error "only function and template parameters can be parameter packs"
//@ diag: 14:21 error "exception type contains unexpanded parameter pack 'T'"
//@ diag: 17:18 error "only function and template parameters can be parameter packs"
//@ diag: 17:22 error "exception type contains unexpanded parameter pack 'T'"
//@ diag: 20:19 error "only function and template parameters can be parameter packs"
//@ diag: 20:22 error "exception type contains unexpanded parameter pack 'T'"
template<class... T> void f() {
  try {} catch (T...) {}
}
template<class... T> void g() {
  try {} catch (T... x) {}
}
template<class... T> void h() {
  try {} catch (T&...) {}
}
