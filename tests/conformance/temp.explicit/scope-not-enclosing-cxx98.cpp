// C++03 [temp.explicit]/5: an explicit instantiation is placed in the
// namespace of its template (C++11 relaxed this to an enclosing
// namespace, which enclosing-namespace-cxx98.cpp keeps accepted in
// C++98). An explicit instantiation in a namespace that does not enclose
// the template is ill-formed under both rules, so it is an error in C++98
// too, as GCC 16.1 reports it (kiln skipped the check before C++11 from
// WP-1473 to WP-1692); clang 22 gives only its off-by-default
// -Wc++11-compat warning. Found running GCC's C++ testsuite (WP-1692).
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ oracle: gcc
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
