// C++03 [temp.explicit]/5: an explicit instantiation is placed in the
// namespace of its template (C++11 relaxed this to an enclosing
// namespace, which enclosing-namespace-cxx98.cpp keeps accepted in
// C++98). An explicit instantiation in a namespace that does not enclose
// the template is ill-formed under both rules, so it is an error in C++98
// too, as GCC 16.1 reports it; since WP-1473 kiln skips the whole check
// before C++11, and clang 22 gives only its off-by-default
// -Wc++11-compat warning. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ xfail(kiln): WP-1691 remainder: the explicit instantiation scope check is skipped in C++98, also for a namespace that does not enclose the template
//@ xfail(clang): clang 22 only warns under -Wc++11-compat in C++98
//@ diag: 23:28 error "explicit instantiation of 'run' not in a namespace enclosing 'Lib'"
//@ diag: 20:10 note "explicit instantiation refers here"
//@ diag: 27:30 error "explicit instantiation of 'run' not in a namespace enclosing 'Lib'"
//@ diag: 20:10 note "explicit instantiation refers here"
namespace Lib {
template <class T> struct Box {
    void run() {}
};
namespace Detail {
template void Box<double>::run();
}
}
namespace Other {
template void Lib::Box<int>::run();
}
