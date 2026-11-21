// Function templates whose declarations hold function types inside other
// declarators, with dependent parameters an address space or a Swift
// parameter ABI applies to (nested-function-params-importer.cpp,
// nested-function-params-importer-calls.cpp).
export module nfp;
#define AS(n) __attribute__((address_space(n)))
#define SWIFT __attribute__((swiftcall))
#define CTX __attribute__((swift_context))
#define ERR __attribute__((swift_error_result))

export template <int N> auto a1() -> void (*)(int AS(N));
export template <class T> void a2(void (*)(T));
export template <class T> void a3(void (*)(T[2]));
export template <class T> using E = void (*)(T);
export template <class... T> auto h3() -> SWIFT void (*)(T CTX..., void **ERR);
export template <class T> auto h4() -> SWIFT void (*)(T CTX);
export template <class T> struct S { using F = SWIFT void (*)(T CTX); };
