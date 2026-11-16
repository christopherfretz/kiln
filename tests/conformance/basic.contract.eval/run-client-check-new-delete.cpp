// -fcontracts-client-check=pre with -fcontracts-definition-check=off
// (spec/codegen.md, "Caller-side contract checks"): GCC 16.1 checks the
// preconditions of the allocation and deallocation functions that new-
// and delete-expressions call, the deallocation of a new-expression
// whose initialization throws included; destructors and constructors
// are not checked by the caller. Found running GCC's C++ testsuite
// (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=pre -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: new
//@ stdout: delete
//@ stdout: new[]
//@ stdout: delete[]
//@ stdout: new(int)
//@ stdout: delete
//@ stdout: new
//@ stdout: delete
//@ stdout: caught
//@ stdout: done
#include <contracts>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
const char *what = "";
void handle_contract_violation(const std::contracts::contract_violation &) {
  std::puts(what);
}
struct T {
  int v = 1;
  T() = default;
  explicit T(int x) pre(x > 0) : v(x) {
    if (x < 0) {
      what = "delete";
      throw 1;
    }
  }
  ~T() pre(v > 0) {}
  static void *operator new(std::size_t n) pre(n > 1000) { return std::malloc(n); }
  static void *operator new(std::size_t n, int k) pre(k > 0) { return std::malloc(n); }
  static void *operator new[](std::size_t n) pre(n > 1000) { return std::malloc(n); }
  static void operator delete(void *p) pre(p == nullptr) { std::free(p); }
  static void operator delete(void *p, int k) pre(k > 0) { std::free(p); }
  static void operator delete[](void *p) pre(p == nullptr) { std::free(p); }
};
int main() {
  what = "new";
  T *p = new T;
  what = "delete";
  delete p;
  what = "new[]";
  T *a = new T[2];
  what = "delete[]";
  delete[] a;
  what = "new(int)";
  p = new (-1) T;
  what = "delete";
  delete p;
  what = "destructor";
  {
    T t;
    t.v = -1;
    t.~T();
    t.v = 1;
  }
  try {
    what = "new";
    p = new T(-1);
  } catch (int) {
    std::puts("caught");
  }
  std::puts("done");
}
