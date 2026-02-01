#!/usr/bin/env bash

HOOK_SCRIPT=""
ORIG_DIR=""
ORIG_PATH=""
TEST_DIR=""

set_up_before_script() {
  HOOK_SCRIPT="$(pwd)/scripts/src/git_hooks/trivy.sh"
}

set_up() {
  TEST_DIR=$(mktemp -d)
  ORIG_DIR="$(pwd)"
  ORIG_PATH="$PATH"
  cd "$TEST_DIR" || exit 1
  git init -q
  git config user.email "test@example.com"
  git config user.name "test"
  mkdir bin
  cat >bin/trivy <<'EOF'
#!/usr/bin/env bash
echo "trivy $*"
EOF
  chmod +x bin/trivy
  export PATH="$TEST_DIR/bin:$PATH"
}

tear_down() {
  export PATH="$ORIG_PATH"
  cd "$ORIG_DIR" || exit 1
  rm -rf "$TEST_DIR"
}

function test_passes_through_when_no_ignored_dirs_exist() {
  touch README.md
  git add README.md >/dev/null 2>&1
  git commit -q -m init
  local output
  output="$("$HOOK_SCRIPT")"
  assert_same "trivy fs ." "$output"
}

function test_includes_single_ignored_dir() {
  echo "build/" >.gitignore
  mkdir build
  touch build/out.js README.md
  git add .gitignore README.md >/dev/null 2>&1
  git commit -q -m init
  local output
  output="$("$HOOK_SCRIPT")"
  assert_same "trivy fs --skip-dirs build ." "$output"
}

function test_joins_multiple_ignored_dirs_with_commas() {
  printf 'build/\ndist/\n' >.gitignore
  mkdir build dist
  touch build/out.js dist/out.js README.md
  git add .gitignore README.md >/dev/null 2>&1
  git commit -q -m init
  local output
  output="$("$HOOK_SCRIPT")"
  assert_contains "build" "$output"
  assert_contains "dist" "$output"
  assert_contains "," "$output"
}

function test_ignores_file_only_gitignore_entries() {
  printf '.DS_Store\nbuild/\n' >.gitignore
  touch .DS_Store
  mkdir build
  touch build/out.js README.md
  git add .gitignore README.md >/dev/null 2>&1
  git commit -q -m init
  local output
  output="$("$HOOK_SCRIPT")"
  assert_same "trivy fs --skip-dirs build ." "$output"
}

function test_handles_nested_ignored_dirs() {
  echo "**/node_modules/" >.gitignore
  mkdir -p packages/a/node_modules packages/b/node_modules
  touch README.md packages/a/node_modules/x packages/b/node_modules/y
  git add .gitignore README.md >/dev/null 2>&1
  git commit -q -m init
  local output
  output="$("$HOOK_SCRIPT")"
  assert_contains "packages/a/node_modules" "$output"
  assert_contains "packages/b/node_modules" "$output"
}
