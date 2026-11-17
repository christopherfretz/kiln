// C++14 [dcl.init]/17.6.3: a copy-initialization by converting constructor
// "initializes a temporary of the cv-unqualified version of the
// destination type", which then direct-initializes the object. For a
// `const T` destination the copy is from a `T` prvalue: the move
// constructor is chosen (over a deleted copy constructor and over a
// forwarding constructor template deduced as `const T&&`, whose
// instantiation would fail here), also for the `const` elements of an
// array and of an initializer list's backing array. Reduced from real code
// (nlohmann_json's `json_ref` in braced initializer lists at C++11).
//@ args: -std=c++11
template <bool B> struct enable_if {};
template <> struct enable_if<true> { using type = int; };

struct Ref {
  Ref(const char*) {}
  Ref(int) {}
  template <class... A, typename enable_if<sizeof...(A) != 1>::type = 0>
  Ref(A&&...) {}
  template <class A>
  Ref(A&&) { static_assert(sizeof(A) == 0, "forwarding constructor instantiated"); }
  Ref(Ref&&) noexcept = default;
  Ref(const Ref&) = delete;
};

const Ref r = "age";
const Ref arr[] = {"name", 1};
void take(const Ref);
void t1() { take("country"); }
const Ref t2() { return 2; }

struct Move {
  Move(int) {}
  Move(Move&&) {}
  Move(const Move&) = delete;
};
const Move m = 1;
const volatile Move mv = 1;
