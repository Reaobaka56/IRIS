# GitHub Linguist Submission for IRIS

This directory contains the materials needed to submit a pull request to
[github/linguist](https://github.com/github/linguist) to get IRIS recognized
and syntax-highlighted on GitHub.

## How to Submit

### Prerequisites

1. Fork [github/linguist](https://github.com/github/linguist)
2. Clone your fork:
   ```bash
   git clone https://github.com/<your-username>/linguist.git
   cd linguist
   ```

### Steps

1. **Add the language entry** to `lib/linguist/languages.yml`:

   Copy the contents of `languages.yml` from this directory and insert it
   alphabetically under the "I" section.

2. **Add the TextMate grammar** as a submodule:

   The grammar lives in the IRIS repository. Add it as a submodule:
   ```bash
   git submodule add https://github.com/moon9t/iris.git vendor/grammars/iris
   ```

   Then add the grammar mapping to `grammars.yml`:
   ```yaml
   vendor/grammars/iris:
     - source.iris
   ```

3. **Add sample files** to `samples/IRIS/`:
   ```bash
   mkdir -p samples/IRIS
   cp /path/to/iris/linguist/samples/* samples/IRIS/
   ```

4. **Run the tests**:
   ```bash
   bundle install
   bundle exec rake test
   ```

5. **Submit the PR** with title: "Add IRIS language"

## Files in This Directory

- `languages.yml` — The entry to add to linguist's languages.yml
- `grammars.yml` — The entry to add to linguist's grammars.yml
- `heuristics.yml` — Disambiguation heuristic (IRIS vs other .iris extensions)
- `samples/` — Sample .iris files for language detection
- `iris.tmLanguage.json` — Symlink/copy reference to the TextMate grammar

## Requirements for Linguist Acceptance

Per [linguist's CONTRIBUTING.md](https://github.com/github/linguist/blob/main/CONTRIBUTING.md):

- [x] Language has a unique file extension (`.iris`)
- [x] Language has a TextMate grammar (`source.iris`)
- [x] Language has sample files demonstrating real usage
- [x] Language is used in repositories on GitHub (the main IRIS repo)
- [x] Language has documentation (BOOK.md, README.md)
