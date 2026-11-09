// [lex.key]/1, [diff.cpp17.lex]/2: 'concept', 'consteval', 'constinit',
// 'co_await', 'co_return', 'co_yield', 'requires' and 'char8_t' are keywords
// only from C++20 on; before that they are ordinary identifiers (clang's
// KEYCXX20 / KEYCOROUTINES / -fchar8_t in TokenKinds.def; WP-1134).
//@ std: c++17
//@ strict-diags
int consteval = 1;
int constinit = 2;
int concept = 3;
int requires(int co_await) { return co_await; }
struct co_yield { int co_return; };
int char8_t = requires(consteval + constinit + concept) + co_yield{4}.co_return;
