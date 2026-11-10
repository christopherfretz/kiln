// [expr.prim.lambda.closure]/3, [dcl.spec.auto] (C++11): a lambda's call
// operator has an implied `auto` return type, not a written placeholder,
// so the C++11 "'auto' return without trailing return type" rule does not
// apply to it, also when the lambda is instantiated with its enclosing
// template (clang `TreeTransform::TransformLambdaExpr`). kiln rebuilt the
// call operator's declarator without its lambda mark, rejecting every
// lambda without a trailing return type in an instantiated template at
// -std=c++11. WP-1213.
//@ expect: run
//@ std: c++11
//@ args: -pedantic
//@ strict-diags
//@ stdout: 3|23|105|3|6
extern "C" int printf(const char *, ...);

template <class F> int take(F f) { return f(2, 3); }

// The reduced case: a lambda without a trailing return type in a
// function template.
template <class S> int f() { return take([](S, S b) { return b; }); }

// A nested lambda.
template <class S> int nested() {
  return take([](S a, S b) {
    auto inner = [](S x) { return x * 10; };
    return inner(a) + b;
  });
}

// A lambda in a member function of a class template.
template <class S> struct Box {
  S v;
  int get() const {
    return take([this](S a, S b) { return v + a + b; });
  }
};

// A lambda in a default argument of a function template.
template <class S>
int defarg(int (*fp)(S, S) = [](S a, S b) { return a - b; }) {
  return fp(7, 4);
}

// A lambda in a member function template of a class template.
template <class S> struct Outer {
  template <class U> static S run(U u) {
    return [u](S s) { return s + static_cast<S>(u); }(S(1));
  }
};

int main() {
  printf("%d|%d|%d|%d|%d\n", f<int>(), nested<int>(), Box<int>{100}.get(),
         defarg<int>(), Outer<int>::run(5L));
}
