// DR1467 ([dcl.init.list]/3.2 as clang applies it to every class, WP-170):
// a list holding one element of the class or a derived class initializes
// the object from that element, before any initializer-list constructor
// is considered: copy-initialized for copy-list-initialization (explicit
// constructors are not candidates), direct-initialized for direct-list.
//@ expect: compile-fail
//@ diag: 20:25 error "no matching constructor for initialization of 'X'"
//@ diag: 17:39 note "no known conversion from 'const X' to"
//@ diag: 17:26 note "explicit constructor is not a candidate"
//@ diag: 17:12 note "requires 0 arguments, but 1 was provided"
//@ diag: 27:18 error "call to deleted constructor of 'Z'"
//@ diag: 26:17 note "'Z' has been explicitly marked deleted here"
//@ requires: clang<22
// clang 20+ (CWG2137) keeps the copy to aggregates: dr1467-single-element.cpp.
#include "Inputs/initializer_list.h"

struct X { X(); explicit X(const X&); X(std::initializer_list<int>); };
X mx();
void t1(const X &x) { X a{x}; }
void t2(const X &x) { X a = {x}; }
void t3() { X a = {mx()}; X b{mx()}; }
struct Y { Y(); Y(std::initializer_list<Y>) = delete; };
struct DY : Y {};
void t4(Y y, DY d) { Y a{y}; Y b = {y}; Y c{d}; Y e = {DY()}; }
void t5(const Y &y) { (void)Y{y}; }
struct Z { Z(); Z(const Z&) = delete; Z(std::initializer_list<Z>); };
void t6(Z z) { Z a{z}; }
struct W {
  int k;
  constexpr W(int i) : k(i) {}
  constexpr W(const W &w) : k(w.k + 10) {}
  constexpr W(std::initializer_list<W> l) : k(100 + int(l.size())) {}
};
constexpr W w0{1};
static_assert(w0.k == 101);
constexpr W w1{w0};
static_assert(w1.k == 111);
constexpr W w2 = {w0};
static_assert(w2.k == 111);
constexpr W w3{W{1}};
static_assert(w3.k == 101);
constexpr W w4{w0, w0};
static_assert(w4.k == 102);
