#!/usr/bin/env bash
# Enforce >= 80% coverage on changed lines over every lcov file in <lcov-dir>.
#
# Usage: diff-cover.sh <lcov-dir> <compare-ref> [<report-dir>]
#
# The same gate as ci-lite.yml's `PR CI Gate`, for the lane-runner flow: the
# ex63 job runs it in place, the hosted flow over the lanes' artifacts.
set -euo pipefail

lcov_dir="${1:?usage: diff-cover.sh <lcov-dir> <compare-ref> [<report-dir>]}"
compare_ref="${2:?usage: diff-cover.sh <lcov-dir> <compare-ref> [<report-dir>]}"
report_dir="${3:-${lcov_dir}}"
mkdir -p "${report_dir}"

mapfile -t lcov_files < <(find "${lcov_dir}" -type f -name '*.info' | sort)
if [ "${#lcov_files[@]}" -eq 0 ]; then
  echo "::error::No lcov files under ${lcov_dir}; the coverage gate cannot run"
  exit 1
fi
printf '[ci][diff-cover] lcov: %s\n' "${lcov_files[@]}"

diff-cover "${lcov_files[@]}" \
  --compare-branch="${compare_ref}" \
  --fail-under=80 \
  --html-report "${report_dir}/diff-coverage.html" \
  --markdown-report "${report_dir}/diff-coverage.md" \
  --format "json:${report_dir}/diff-coverage.json"

# diff-cover exits 0 when it measured nothing at all, which is how #5593 passed
# with 1,643 uncompiled lines. That is legitimate for a diff whose only code
# changes are comments, imports or declarations, so this only warns; the hard
# "never compiled" failure is scripts/ci/assert-coverage-presence.sh in the
# Rust coverage check.
measured="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["total_num_lines"])' "${report_dir}/diff-coverage.json")"
echo "[ci][diff-cover] measured ${measured} changed line(s)"
if [ "${measured}" -eq 0 ]; then
  echo "::warning::diff-cover measured 0 changed lines. If this PR changed executable code, no coverage check compiled it — see the rust-core-coverage log for the coverage-presence gate."
fi
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f "${report_dir}/diff-coverage.md" ]; then
  cat "${report_dir}/diff-coverage.md" >> "${GITHUB_STEP_SUMMARY}"
fi
