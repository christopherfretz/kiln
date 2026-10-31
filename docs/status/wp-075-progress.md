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
