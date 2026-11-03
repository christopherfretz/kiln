// [dcl.init.list]/3.2 as clang 20+ applies DR1467 and CWG2137: only an
// aggregate is initialized from a single element of its class (or a
// derived class); any other class goes through [over.match.list], the
// initializer-list constructors first, then all constructors, where a
// prvalue of the class itself initializes the object directly
// ([dcl.init.general]/16.6.1) and an explicit constructor chosen for
// copy-list-initialization is an error.
//@ expect: compile-fail
//@ requires: clang>=22
//@ diag: 33:25 error "chosen constructor is explicit in copy-initialization"
//@ diag: 30:26 note "explicit constructor declared here"
//@ diag: 37:24 error "call to deleted constructor of 'Y'"
//@ diag: 35:17 note "'Y' has been explicitly marked deleted here"
//@ diag: 37:32 error "call to deleted constructor of 'Y'"
//@ diag: 35:17 note "'Y' has been explicitly marked deleted here"
//@ diag: 37:43 error "call to deleted constructor of 'Y'"
//@ diag: 35:17 note "'Y' has been explicitly marked deleted here"
//@ diag: 37:51 error "call to deleted constructor of 'Y'"
//@ diag: 35:17 note "'Y' has been explicitly marked deleted here"
//@ diag: 38:29 error "call to deleted constructor of 'Y'"
//@ diag: 35:17 note "'Y' has been explicitly marked deleted here"
//@ diag: 40:20 error "call to deleted constructor of 'const Z'"
//@ diag: 39:17 note "'Z' has been explicitly marked deleted here"
//@ diag: 50:15 error "static assertion failed due to requirement 'w1.k == 111'"
//@ diag: 50:20 note "expression evaluates to '101 == 111'"
//@ diag: 52:15 error "static assertion failed due to requirement 'w2.k == 111'"
//@ diag: 52:20 note "expression evaluates to '101 == 111'"
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
struct E { E(); explicit E(const E&); };
E me();
void t7(const E &e) { E a{e}; E b = {me()}; E c{me()}; }
struct P { P(); P(const P&) = delete; };
P mp();
void t8() { P a{mp()}; P b = {mp()}; }
struct Q { Q(); explicit Q(const Q&); Q(std::initializer_list<int>); operator int() const; };
void t9(const Q &q) { Q a = {q}; Q b{q}; }
