# WP-075 progress — lambdas

Branch `wp/075-lambdas`, worktree `kiln/.worktrees/wp-075`.

## Baseline (base = main at 1373/1386 kiln-mode)
Header probe (`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`):
`<array>` 103 errors, `<string_view>` 167, `<span>` 76.

## Steps
1. [x] Non-capturing lambdas end to end (parse, sema closure class, Oven, codegen, dump)
2. [x] Captures (except packs and braced/paren init-captures)
3. [x] Generic lambdas
4. [x] Lambdas in templates (LambdaExpr substitution), generic lambdas in templates
5. [x] spec/sema-lambda.md, ast.md §5.9, abi.md §8.9
6. [x] Tests (46 conformance, 4 codegen, xtask clang-ast LambdaExpr)

## Log
- WIP 1: parser (`kiln_parse/src/lambda.rs`: introducer, specifiers, declarator,
  body; clang ParseLambda* diagnostics), sema (`kiln_sema/src/lambda.rs`:
  closure record, constexpr inline `operator()`, function-pointer conversion,
  `__invoke`, deleted default ctor/copy-assign with captures, lambda falloff
  wording, referenced trivial dtor). Dump of non-capturing forms matches clang
  19 including implicit members. Captures/generic/template-param lists still
  `unsupported`. Oven/codegen not started.
- WIP 2-4: captureless codegen (closure non-POD, conversion + `__invoke`,
  unique-external linkage, printQualifiedName IR names), Oven call through the
  closure, lambda diag names (`lambda at`), clang-ast LambdaExpr printer +
  ast goldens, 24 conformance tests. Step 1 done: `ci --quick` green at
  41b462b (1395/1410, lambdas-in-templates xfail).
- WIP 5-8: captures. Sema: explicit/implicit/init/`this`/`*this` captures,
  clang `tryCaptureVariable` chain through nested lambdas, odr-use capture of
  constant-usable variables at full-expression end, impcap/const/this
  diagnostics with clang's notes, closure members and deleted special
  members. Codegen: closure field lvalues, captured `this`, EH cleanups for
  capture members and captured arrays (matches clang -fexceptions). Oven:
  closure member places, captured `this`/`*this`. Merged main at 19aa409.
  Step 2 done: `ci --quick` green at 39e190d (1526/1542, 35 lambda tests).
- WIP 9-11: generic lambdas. Call operator is a FunctionTemplate (invented
  `auto` parameters + explicit template-parameter-list + requires-clause);
  conversion function and `__invoke` are FunctionTemplates sharing its
  parameter list; conversion deduction (P = `auto(*)(T...)`, return type
  taken from the target) then specializes the call operator to deduce its
  return type and types the conversion/invoker specs from it (mismatch fails
  the conversion sequence like clang). decltype(auto) mangles `Dc`;
  dependent void return deduction deferred to instantiation; local-scope
  qualified names in instantiation notes (`f()::(anonymous class)::...`).
  42 lambda tests (40 pass, 2 xfail: lambdas in templates).
- Step 3 done. Merged main at 25785be.
- WIP 12-13: lambdas in templates (`templ/subst/lambda.rs`): the lambda is
  replayed through the parser's actions per instantiation (explicit
  captures with substituted init-captures, declarator, body; implicit and
  `this` captures found again). Generic lambdas in templates: substituted
  template parameter list (`open_substituted_head`), pre-resolved simple
  captures (`LambdaCaptureSpec.var`), retained function parameter packs,
  constraints through `member_constraints`. `ci --quick` green at ca79afb
  (1561/1577; 46 lambda tests, 1 xfail).
- Step 4-6 done: spec/sema-lambda.md, ast.md §5.9, abi.md §8.9.

## Result
Header probe after: `<array>` 78, `<string_view>` 142, `<span>` 59
(main's binary at handback: 82 / 146 / 61).
