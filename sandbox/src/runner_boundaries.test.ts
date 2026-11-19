import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runLighthouse } from './lighthouse_runner.js';
import { runLocust, parseLocustStats } from './locust_runner.js';
import { runZapScan } from './zap_runner.js';

const csv = 'Type,Name,Request Count,Failure Count,Median Response Time,Average Response Time,95%,99%,Requests/s\n,Aggregated,10,1,3,4,8,9,2\n';
test('Locust requires measured totals and every percentile rather than synthesizing zero', () => {
  assert.deepEqual(parseLocustStats(csv, 5), { total_requests: 10, total_failures: 1, median_response_time: 3, avg_response_time: 4, p95_response_time: 8, p99_response_time: 9, rps: 2, duration: 5 });
  for (const invalid of ['', csv.replace('Aggregated', 'endpoint'), csv.replace(',10,1,', ',0,0,'), csv.replace(',8,9,', ',,9,'), csv.replace(',8,9,', ',NaN,9,')]) {
    assert.throws(() => parseLocustStats(invalid, 5));
  }
});
test('runners fail closed without installed tools; URL text never becomes shell code', { skip: process.platform === 'win32' }, async () => {
  const directory = await realpath(await mkdtemp(join(tmpdir(), 'qinaegis-runner-boundary-')));
  const previousCwd = process.cwd(), previousPath = process.env.PATH;
  try {
    process.chdir(directory);
    await mkdir('bin');
    process.env.PATH = join(directory, 'bin');
    await assert.rejects(runLighthouse('http://127.0.0.1/', join(directory, 'report.json')), /unavailable or failed/);
    await assert.rejects(runLocust('http://127.0.0.1/', 1, 1, 1), /unavailable/);
    await assert.rejects(runZapScan('http://127.0.0.1/', join(directory, 'zap.json')));
    await assert.rejects(runLocust('http://127.0.0.1/', 1001, 1, 1), /limits/);
    const script = `#!${process.execPath}\nconst fs=require('fs');fs.writeFileSync('args.json',JSON.stringify(process.argv.slice(2)));fs.writeFileSync('report.json',JSON.stringify({categories:{performance:{score:.9},accessibility:{score:1},'best-practices':{score:1},seo:{score:1}},audits:Object.fromEntries(['first-contentful-paint','largest-contentful-paint','cumulative-layout-shift','total-blocking-time','speed-index','server-response-time'].map(k=>[k,{numericValue:1}]))}));`;
    await writeFile('bin/lighthouse', script, { mode: 0o700 });
    const target = 'http://127.0.0.1/?x=;touch injected';
    assert.equal((await runLighthouse(target, join(directory, 'report.json'))).score, .9);
    assert.equal(JSON.parse(await readFile('args.json', 'utf8'))[0], target);
    await assert.rejects(readFile('injected'));
    await writeFile('bin/lighthouse', `#!${process.execPath}\nrequire('fs').writeFileSync('report.json','{}');`, { mode: 0o700 });
    await assert.rejects(runLighthouse(target, join(directory, 'report.json')), /metrics missing/);
    await writeFile('bin/locust', `#!${process.execPath}\nrequire('fs').writeFileSync('locust-metrics_stats.csv',${JSON.stringify(csv)});`, { mode: 0o700 });
    assert.equal((await runLocust('http://127.0.0.1/', 1, 1, 1)).stats.total_requests, 10);
    await writeFile('bin/locust', `#!${process.execPath}\nconsole.log('Running Locust');process.exit(1);`, { mode: 0o700 });
    await assert.rejects(runLocust('http://127.0.0.1/', 1, 1, 1), /execution failed/);
  } finally {
    process.chdir(previousCwd);
    if (previousPath === undefined) delete process.env.PATH; else process.env.PATH = previousPath;
    await rm(directory, { recursive: true, force: true });
  }
});
