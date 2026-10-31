# WP-075 progress — lambdas

Branch `wp/075-lambdas`, worktree `kiln/.worktrees/wp-075`.

## Baseline (base = main at 1373/1386 kiln-mode)
Header probe (`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`):
`<array>` 103 errors, `<string_view>` 167, `<span>` 76.

## Steps
1. [ ] Non-capturing lambdas end to end (parse, sema closure class, Oven, codegen, dump)
2. [ ] Captures
3. [ ] Generic lambdas
4. [ ] spec/sema-lambda.md, ast.md/abi.md rows
5. [ ] Tests (conformance >= 40, codegen corpus, xtask clang-ast LambdaExpr)

## Log
- WIP 1: parser (`kiln_parse/src/lambda.rs`: introducer, specifiers, declarator,
  body; clang ParseLambda* diagnostics), sema (`kiln_sema/src/lambda.rs`:
  closure record, constexpr inline `operator()`, function-pointer conversion,
  `__invoke`, deleted default ctor/copy-assign with captures, lambda falloff
  wording, referenced trivial dtor). Dump of non-capturing forms matches clang
  19 including implicit members. Captures/generic/template-param lists still
  `unsupported`. Oven/codegen not started.
