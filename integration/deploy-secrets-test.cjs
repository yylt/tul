'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const workflow = fs.readFileSync(path.join(__dirname, '../.github/workflows/deploy.yml'), 'utf8');
const step = workflow.split('      - name: Update environments\n')[1].split('\n      - name:')[0];
const script = step.split('        run: |\n')[1]
  .split('\n').map(line => line.replace(/^          /, '')).join('\n')
  .replace(/\$\{\{ secrets\.[A-Z_]+ \}\}/g, '');

for (const [name, value, existing, shouldPut] of [
  ['sets MAPURL from GitHub when absent in Worker', 'https://example.invalid/?token=a&b="c"', false, true],
  ['skips MAPURL when GitHub secret is absent', '', false, false],
  ['preserves existing Worker MAPURL', 'https://example.invalid/', true, false]
]) {
  test(name, () => {
    assert.match(step, /MAPURL: \$\{\{ secrets\.MAPURL \}\}/);
    const mock = `
set -e
npx() {
  if [[ "$*" == "wrangler secret list" ]]; then
    if [[ "$EXISTING" == 1 ]]; then printf '[{"name":"MAPURL"}]'; else printf '[]'; fi
  elif [[ "$*" == "wrangler secret put MAPURL" ]]; then
    local value
    IFS= read -r value
    [[ "$value" == "$MAPURL" ]] || return 1
    printf 'MOCK_MAPURL_SET'
  else
    return 1
  fi
}
`;
    const result = spawnSync('bash', ['-c', mock + script], {
      encoding: 'utf8', env: { ...process.env, MAPURL: value, EXISTING: existing ? '1' : '0' }
    });
    assert.equal(result.status, 0, result.stderr + result.stdout);
    assert.equal(result.stdout.includes('MOCK_MAPURL_SET'), shouldPut);
  });
}
