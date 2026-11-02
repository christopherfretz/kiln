// [dcl.fct.def.coroutine]/4: the promise type is
// std::coroutine_traits<R, P...>::promise_type; a specialization without
// that member makes the function ill-formed as a coroutine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:5 error "this function cannot be a coroutine: 'std::coroutine_traits<job>' has no member named 'promise_type'"
namespace std {
template <class R, class... A> struct coroutine_traits {};
} // namespace std

struct job {};

job f() { co_return; }
