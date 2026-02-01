#!/usr/bin/env bash

HOOK_SCRIPT=""

set_up_before_script() {
  HOOK_SCRIPT="$(pwd)/scripts/src/git_hooks/forbid_raw_raster_img.sh"
}

set_up() {
  TEST_DIR=$(mktemp -d)
  mkdir -p "${TEST_DIR}/apps/landing/src/components"
  # The hook greps `apps/landing/src` relative to cwd. Run from the temp dir.
  ORIG_DIR="$(pwd)"
  cd "$TEST_DIR" || exit 1
}

tear_down() {
  cd "$ORIG_DIR" || exit 1
  rm -rf "$TEST_DIR"
}

function test_passes_when_no_astro_files_exist() {
  "$HOOK_SCRIPT"
  assert_exit_code "0"
}

function test_passes_when_astro_file_has_no_img_tags() {
  cat >apps/landing/src/components/plain.astro <<'EOF'
---
const title = "hello";
---
<h1>{title}</h1>
EOF

  "$HOOK_SCRIPT"
  assert_exit_code "0"
}

function test_passes_when_only_svg_img_references_exist() {
  cat >apps/landing/src/components/logo.astro <<'EOF'
---
---
<img src="/logo.svg" width="96" height="39" alt="logo" />
EOF

  "$HOOK_SCRIPT"
  assert_exit_code "0"
}

function test_fails_on_raw_png_img_tag() {
  cat >apps/landing/src/components/hero.astro <<'EOF'
---
---
<img src="/photo.png" width="600" height="400" alt="hero" />
EOF

  local output
  output=$("$HOOK_SCRIPT" 2>&1)
  assert_exit_code "1"
  assert_contains "apps/landing/src/components/hero.astro" "$output"
  assert_contains "<Image>" "$output"
}

function test_fails_on_raw_jpg_img_tag() {
  cat >apps/landing/src/components/cover.astro <<'EOF'
---
---
<img src="/cover.jpg" alt="cover" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}

function test_fails_on_raw_jpeg_img_tag() {
  cat >apps/landing/src/components/cover.astro <<'EOF'
---
---
<img src="/cover.jpeg" alt="cover" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}

function test_fails_on_raw_webp_img_tag() {
  cat >apps/landing/src/components/cover.astro <<'EOF'
---
---
<img src="/cover.webp" alt="cover" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}

function test_fails_on_raw_avif_img_tag() {
  cat >apps/landing/src/components/cover.astro <<'EOF'
---
---
<img src="/cover.avif" alt="cover" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}

function test_passes_on_data_url_img_tag() {
  # Data URL PNGs are intentional (inline placeholders, tiny icons).
  cat >apps/landing/src/components/placeholder.astro <<'EOF'
---
---
<img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA" alt="placeholder" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "0"
}

function test_fails_when_single_quotes_are_used() {
  cat >apps/landing/src/components/quote.astro <<EOF
---
---
<img src='/banner.png' alt='banner' />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}

function test_ignores_non_astro_files() {
  # The hook is scoped to .astro; PNG references elsewhere stay quiet.
  mkdir -p apps/landing/src/scripts
  cat >apps/landing/src/scripts/thing.ts <<'EOF'
const url = "/photo.png";
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "0"
}

function test_ignores_files_outside_apps_landing_src() {
  # Public assets are allowed to be referenced by stable paths from HTML meta
  # tags. Only src/ is covered by the policy.
  mkdir -p apps/landing/public
  cat >apps/landing/public/og.png <<'EOF'
binary placeholder
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "0"
}

function test_fails_when_attributes_surround_src() {
  # Realistic tag: attrs before and after src.
  cat >apps/landing/src/components/attrs.astro <<'EOF'
---
---
<img class="w-full" loading="lazy" src="/hero.png" alt="hero" width="800" height="600" />
EOF

  "$HOOK_SCRIPT" >/dev/null 2>&1
  assert_exit_code "1"
}
