#!/usr/bin/env node
// Build the copyable starter and exercise its relocated packages without a GPU.
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const assert = require('node:assert/strict');

const root = path.resolve(__dirname, '..');
const scratch = path.join(root, 'target', 'cljs-package-test');
const project = path.join(scratch, 'app source');
const host = process.env.CLJ_GPUI_BIN || path.join(root, 'host/target/debug/clj-gpui');
fs.accessSync(host, fs.constants.X_OK);
fs.mkdirSync(project, {recursive: true});
fs.cpSync(path.join(root, 'templates/cljs'), project, {
  recursive: true,
  filter: file => !['node_modules', 'target', '.shadow-cljs', '.cpcache'].includes(path.basename(file)),
});
const deps = path.join(project, 'deps.edn');
fs.writeFileSync(deps, fs.readFileSync(deps, 'utf8').replace('"../clj-gpui"', JSON.stringify(root)));

function run(command, args, options = {}) {
  const result = cp.spawnSync(command, args, {
    cwd: project, stdio: 'inherit', timeout: 600000,
    env: {...process.env, CLJ_GPUI_BIN: host}, ...options,
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${command} ${args.join(' ')} failed (${result.signal})`);
}

run('npm', ['ci', '--no-audit', '--no-fund']);
// Subsequent invocations must replace stale payload files without manual cleanup.
const stale = path.join(project, 'target/cljs-app/stale-from-previous-package');
if (fs.existsSync(path.dirname(stale))) fs.writeFileSync(stale, 'must not ship');
run('clojure', ['-X:build']);
assert(!fs.existsSync(stale), 'packaging must rebuild a clean production payload');
const packages = path.join(project, 'target/package');
const moved = path.join(scratch, "relocated app's packages");
fs.rmSync(moved, {recursive: true, force: true});
fs.mkdirSync(moved, {recursive: true});

function checkPayload(base, launcher, hostFile) {
  const app = path.join(base, 'app');
  const node = path.join(base, 'runtime/bin/node');
  fs.accessSync(node, fs.constants.X_OK);
  fs.accessSync(hostFile, fs.constants.X_OK);
  assert(fs.existsSync(path.join(base, 'runtime/LICENSE')));
  assert(fs.existsSync(path.join(app, 'resources/icon.png')));
  assert(!fs.readFileSync(path.join(app, 'main.cjs'), 'utf8').includes(root),
    'release JavaScript must omit the compile-time development checkout path');
  assert(fs.existsSync(path.join(app, 'node_modules/dayjs/dayjs.min.js')));
  assert(!fs.existsSync(path.join(app, 'node_modules/shadow-cljs')));
  assert(!fs.existsSync(path.join(base, 'runtime/bin/java')));
  // Only the relocated test copy gets a protocol peer in place of the GPU host.
  fs.copyFileSync(path.join(__dirname, 'fixtures/template-host.cjs'), hostFile);
  fs.chmodSync(hostFile, 0o755);
  run(launcher, ['argument with spaces'], {
    cwd: scratch, timeout: 30000,
    env: {...process.env, PATH: '/usr/bin:/bin', NODE_PATH: '',
      CLJ_GPUI_BIN: '/invalid/external/host',
      CLJ_GPUI_PACKAGE_EXPECT_NODE: node,
      CLJ_GPUI_PACKAGE_EXPECT_APP: app},
  });
}

if (process.platform === 'darwin') {
  const app = path.join(moved, 'My Relocated App.app');
  fs.cpSync(path.join(packages, 'my-app.app'), app, {recursive: true, verbatimSymlinks: true});
  run('plutil', ['-lint', path.join(app, 'Contents/Info.plist')]);
  checkPayload(path.join(app, 'Contents/Resources'),
    path.join(app, 'Contents/MacOS/my-app'), path.join(app, 'Contents/MacOS/clj-gpui-host'));
} else if (process.platform === 'linux') {
  const appimage = fs.readdirSync(packages).find(file => file.endsWith('.AppImage'));
  const deb = fs.readdirSync(packages).find(file => file.endsWith('.deb'));
  assert(appimage && deb, 'both Linux packages must be present');
  run(path.join(packages, appimage), ['--appimage-extract'], {cwd: moved, stdio: 'pipe'});
  const appdir = path.join(moved, 'squashfs-root');
  checkPayload(path.join(appdir, 'usr'), path.join(appdir, 'AppRun'),
    path.join(appdir, 'usr/bin/clj-gpui-host'));
  const debdir = path.join(moved, 'debian');
  run('dpkg-deb', ['--extract', path.join(packages, deb), debdir]);
  const base = path.join(debdir, 'usr/lib/my-app');
  checkPayload(base, path.join(base, 'bin/my-app'), path.join(base, 'bin/clj-gpui-host'));
} else {
  throw new Error('Package smoke tests support macOS and Linux.');
}
console.log('ClojureScript template packaging and relocated launch passed.');
