# Code and Documentation Writing Guidelines

This file describes the guidelines to follow when you write code comments and documentation.
The goal is comments that are clear, concise, and helpful for understanding the code. Avoid the
extreme verbosity and word salad that modern LLMs produce.

## General Writing Principles

Write all prose in the spirit of ASD-STE100 Simplified Technical English. This applies to
documentation, READMEs, comments, error messages, and anything else a person reads. Do not follow
the rules of ASD-STE100 exactly. Keep these guidelines in mind:

- Words
  - Use one name for one thing. Do not call the same item by two different names.
  - Use the short common word: start (not begin, commence, initiate), use (not utilize, leverage),
    help (not facilitate), make sure (not ensure), before (not prior to), after (not subsequent
    to), about (not regarding, concerning), get (not obtain, acquire), show (not demonstrate),
    also (not additionally, furthermore, moreover).
  - Give each word one meaning. "fall" means to move down, not to decrease.
  - No marketing adjectives: seamless, robust, powerful, cutting-edge, effortless, world-class,
    next-generation, revolutionary.
  - American spelling.
- Verbs
  - Active voice. "the parser reads the file", not "the file is read by the parser".
  - Use a verb for an action. "analyze the log", not "perform an analysis of the log".
  - No stacked auxiliaries. Not "it is important to note that this may help to improve". Write
    "this improves X".
  - No "-ing" main verb where a simple tense works.
- Keep sentence lengths reasonable. Include only one instruction per sentence.
- Do not use semicolons, dashes, or other complex punctuation. Split the sentence instead.
- Do not be afraid to write several paragraphs to explain a complex idea. Each paragraph covers a
  single topic. Put an empty line between paragraphs.

Do not look up the actual text of ASD-STE100. It is copyrighted, and it holds little that these
guidelines do not already cover.

## Contents of Comments

A comment is timeless. It never refers to how something was done in the past, because that code is
irrelevant to the current implementation. The one exception is a hack, a workaround, or a
backwards-compatible code path whose only motivation is a prior implementation. Keep a comment
that refers to past behavior only when another maintainer would be tempted to refactor the code
back to the original flawed implementation without that context.

Never justify or contrast a committed comment against a git-ignored or otherwise local-only
artifact: a `.local/` path, a scratch file, tool output, a local test file. Every comment makes
sense to a person who checks out the repository cleanly and builds it without those artifacts. Do
not add the artifacts to the repo as a way around this requirement.

Remove a comment that provides no use. Do not be afraid to leave code uncommented rather than
clutter it with unhelpful comments. This holds above all for an internal function or an
implementation detail that explains itself.

## Specific Comment Guidelines

- A `module` or top-of-file comment explains the overarching purpose and the high-level design of
  the code. It does not refer to an actual implementation such as a specific function or type. It
  covers the big picture and the rationale behind the code structure. Write it for a person who
  intends to work on the code inside the module, not for a person who uses the module. Keep it to
  a one or two line summary of what is inside the file when there are no complex design decisions.
  Avoid ASCII diagrams.
- A `declaration` comment is written for the *consumer* of the API. It explains how to use the
  function, type, or variable correctly. It holds no implementation detail and no internal
  rationale, unless a consumer needs that information for correct or efficient use. Include an
  example only when the usage is non-obvious or needs clarification.
- A `body` comment splits the implementation into logical sections, and only in a function with
  several distinct steps. A `body` comment never describes what the code does. The code itself is
  clear about that. Include only non-obvious rationale or context.
- A `trailing` comment describes something on that line alone. Keep it small and concise: all
  lowercase, a few words at most, and no trailing period.

## Relationship to CLAUDE.md

`CLAUDE.md` holds the documentation rules for PRDs, specs, ADRs, and tickets. Its declarative
prose rule covers doc comments as well: a sentence states one fact about the subject as it is,
with no temporal anchor and no causal connective. Write a rationale as the fact that grounds the
rule, next to the rule.
