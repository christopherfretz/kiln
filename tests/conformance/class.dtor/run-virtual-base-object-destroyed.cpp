// An object whose class has a virtual base (directly, as an array
// element, as a member or as a base) is destroyed at the end of its
// lifetime even when the class's implicit destructor is constexpr
// (C++26 allows virtual bases in constexpr classes): destroying the
// virtual base runs its non-constexpr destructor, so the object has no
// constant destruction ([expr.const]/7) and the destructor is called.
// kiln dropped the call for automatic, `const` and `static` variables in
// C++26. Found running GCC's C++ testsuite.
//@ std: c++26
//@ expect: run
//@ stdout: 1 3 4 5 6 7 7 8
#include <cstdio>

int n;
struct Foo {
    ~Foo() { ++n; }
};
struct Bar : virtual Foo {};
struct Wrap {
    Bar b;
};
struct Derived : Bar {};

void one() { Bar b; }
void array() { Bar a[2]; }
void member() { Wrap w; }
void base() { Derived d; }
void constant() { const Bar c; }
void statik() { static Bar s; }

// Constructed before `statik`'s `s`, so destroyed after it.
struct Report {
    ~Report() { std::printf(" %d\n", n); }
} report;

int main() {
    one();
    std::printf("%d", n);
    array();
    std::printf(" %d", n);
    member();
    std::printf(" %d", n);
    base();
    std::printf(" %d", n);
    constant();
    std::printf(" %d", n);
    Bar *p = new Bar;
    delete p;
    std::printf(" %d", n);
    statik();
    std::printf(" %d", n);
    return 0;
}
