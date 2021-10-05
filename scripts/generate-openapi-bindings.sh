#!/usr/bin/env bash

set -e

SCRIPTDIR=$(dirname "$0")
TARGET="$SCRIPTDIR/../openapi"
RUST_FMT="$SCRIPTDIR/../.rustfmt.toml"
CARGO_TOML="$TARGET/Cargo.toml"
SPEC="$SCRIPTDIR/../control-plane/rest/openapi-specs/v0_api_spec.yaml"

# Regenerate the bindings only if the rest src changed
check_spec="no"
# Use the Cargo.toml from the openapi-generator
default_toml="no"

case "$1" in
    --changes)
        check_spec="yes"
        ;;
    --default-toml)
        default_toml="yes"
        ;;
esac

if [[ $check_spec = "yes" ]]; then
    git diff --cached --exit-code "$SPEC" 1>/dev/null && exit 0
fi

tmpd=$(mktemp -d /tmp/openapi-gen-XXXXXXX)

# Generate a new openapi crate
openapi-generator-cli generate -i "$SPEC" -g rust-mayastor -o "$tmpd" --additional-properties actixWebVersion="4.0.0-beta.8" --additional-properties actixWebTelemetryVersion='"0.11.0-beta.4"'

if [[ $default_toml = "no" ]]; then
  cp "$CARGO_TOML" "$tmpd"
fi

# Format the files
# Note, must be formatted on the tmp directory as we've ignored the autogenerated code within the workspace
cp "$RUST_FMT" "$tmpd"
( cd "$tmpd" && cargo fmt --all )

# Cleanup the existing autogenerated code
if [ ! -d "$TARGET" ]; then
  mkdir -p "$TARGET"
else
  rm -rf "$TARGET"
  mkdir -p "$TARGET"
fi

rm -rf "$tmpd"/api
mv "$tmpd"/* "$TARGET"/
rm -rf "$tmpd"


# If the openapi bindings were modified then fail the check
git diff --exit-code "$TARGET"
