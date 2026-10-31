//@ expect: compile-fail
//@ diag: 9:9 error "call to 'f' is ambiguous"
//@ diag: 7:25 note "candidate function [with T = int]"
//@ diag: 8:25 note "candidate function [with T = int]"
// [temp.over.link]/5: `decltype(t + 1)` and `decltype(t + 2)` are not
// equivalent, so these declare two overloaded templates.
template <class T> auto f(T t) -> decltype(t + 1);
template <class T> auto f(T t) -> decltype(t + 2);
int r = f(1);
