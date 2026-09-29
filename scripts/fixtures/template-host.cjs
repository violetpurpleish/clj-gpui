#!/usr/bin/env bun
// Real released template + bundled Bun, with a headless protocol peer.
const fs = require('node:fs');
const net = require('node:net');
const readline = require('node:readline');
const assert = require('node:assert/strict');
const path = require('node:path');
const {createRequire} = require('node:module');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(process.env.CLJ_GPUI_PACKAGE_EXPECT_BUN));
assert.equal(fs.realpathSync(process.cwd()), fs.realpathSync(process.env.CLJ_GPUI_PACKAGE_EXPECT_APP));
assert.equal(process.env.NODE_ENV, 'production');
assert.equal(process.env.CLJ_GPUI_APP_HOME, process.env.CLJ_GPUI_PACKAGE_EXPECT_APP);
const appRequire = createRequire(path.join(process.cwd(), 'main.cjs'));
assert(appRequire.resolve('dayjs').startsWith(path.join(process.cwd(), 'node_modules') + path.sep));
const socket = net.connect(Number(process.env.CLJ_GPUI_PORT), process.env.CLJ_GPUI_HOST);
const timer = setTimeout(() => { throw new Error('Packaged template protocol timed out'); }, 10000);
const lines = readline.createInterface({input: socket});
function send(message) { socket.write(JSON.stringify(message) + '\n'); }
function nodes(tree) { return [tree, ...(tree.children || []).flatMap(nodes)]; }
lines.on('line', line => {
  const msg = JSON.parse(line);
  if (msg.op === 'ready') {
    assert.equal(msg['protocol-version'], 11);
    assert.equal(msg.nrepl, 0);
    send({op: 'render', id: 1});
  } else if (msg.id === 1) {
    assert.equal(msg.ok, true);
    assert.equal(msg.tree.chrome, 'app');
    const tree = nodes(msg.tree);
    assert(tree.some(node => node.text === 'Clicks: 0'));
    const button = tree.find(node => node.type === 'button');
    assert(button && button['on-click']);
    send({op: 'callback', id: 2, 'callback-id': button['on-click']});
  } else if (msg.id === 2) {
    assert.equal(msg.ok, true);
    send({op: 'render', id: 3});
  } else if (msg.id === 3) {
    assert.equal(msg.ok, true);
    const tree = nodes(msg.tree);
    assert(tree.some(node => node.text === 'Clicks: 1'));
    assert(tree.some(node => /^Updated at \d{2}:\d{2}:\d{2} with dayjs$/.test(node.text)));
    console.log('Packaged template callback and npm dependency passed.');
    clearTimeout(timer);
    socket.end();
  }
});
