#!/bin/bash
# Verify the "skip already validly signed" filter both ways, without Windows.
#
# The filter is the only new logic in the bundle scripts, and getting it backwards
# would either leave our own binaries unsigned or keep paying to re-sign vendor
# ones. So exercise it against a stub signtool that reports a valid signature for
# exactly the files a vendor would have signed.
set -euo pipefail

work="$(mktemp -d)"
bin="$work/bin"
mkdir -p "$bin" "$work/pkg/bin"

# Stub: `signtool verify -pa <file>` succeeds only for the MSVC redistributables,
# which is what Microsoft ships pre-signed.
cat > "$bin/signtool" <<'STUB'
#!/bin/bash
file="${!#}"
case "$(basename "$file")" in
  vcruntime140.dll|msvcp140.dll|concrt140.dll) exit 0 ;;
  *) exit 1 ;;
esac
STUB
chmod +x "$bin/signtool"
export PATH="$bin:$PATH"

SIGN_DIR="$work/pkg"
touch "$SIGN_DIR/bin/FreeCAD.exe" \
      "$SIGN_DIR/bin/freecadcmd.exe" \
      "$SIGN_DIR/bin/Qt6Core.dll" \
      "$SIGN_DIR/bin/vtkCommonCore.dll" \
      "$SIGN_DIR/bin/vcruntime140.dll" \
      "$SIGN_DIR/bin/msvcp140.dll" \
      "$SIGN_DIR/bin/concrt140.dll" \
      "$SIGN_DIR/bin/_bootlocale.pyd"

shopt -s nullglob
FILES=("$SIGN_DIR"/*.exe "$SIGN_DIR"/bin/*.exe "$SIGN_DIR"/bin/*.dll "$SIGN_DIR"/bin/*.pyd)

TO_SIGN=()
skipped=0
for f in "${FILES[@]}"; do
  if signtool verify -pa "$f" >/dev/null 2>&1; then
    skipped=$((skipped + 1))
  else
    TO_SIGN+=("$f")
  fi
done

echo "matched  ${#FILES[@]}"
echo "skipped  $skipped"
echo "to sign  ${#TO_SIGN[@]}"
printf '  sign: %s\n' "${TO_SIGN[@]##*/}"

fail=0
[[ ${#FILES[@]} -eq 8 ]] || { echo "FAIL: expected 8 matched"; fail=1; }
[[ $skipped -eq 3 ]] || { echo "FAIL: expected 3 skipped (the MSVC redistributables)"; fail=1; }
[[ ${#TO_SIGN[@]} -eq 5 ]] || { echo "FAIL: expected 5 to sign"; fail=1; }

# Our own binaries and the unsigned conda libraries must all still be signed.
for required in FreeCAD.exe freecadcmd.exe Qt6Core.dll vtkCommonCore.dll _bootlocale.pyd; do
  found=0
  for f in "${TO_SIGN[@]}"; do
    [[ "$(basename "$f")" == "$required" ]] && found=1
  done
  [[ $found -eq 1 ]] || { echo "FAIL: $required would NOT be signed"; fail=1; }
done

# And no pre-signed vendor file may appear in the sign list.
for f in "${TO_SIGN[@]}"; do
  case "$(basename "$f")" in
    vcruntime140.dll|msvcp140.dll|concrt140.dll)
      echo "FAIL: $(basename "$f") would be re-signed"; fail=1 ;;
  esac
done

if [[ $fail -eq 0 ]]; then
  echo "PASS: vendor-signed files skipped, everything else still signed"
else
  exit 1
fi
