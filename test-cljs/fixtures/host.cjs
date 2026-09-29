#!/usr/bin/env node
// Small host peer for launcher lifecycle and TCP framing regressions.
const net = require('node:net');
const readline = require('node:readline');
const assert = require('node:assert/strict');
const mode = process.env.CLJ_GPUI_TEST_MODE;
if (mode === 'exit') process.exit(7);
if (mode === 'timeout') {
  setInterval(() => {}, 1000);
} else {
  assert.equal(process.env.CLJ_GPUI_HOST, '127.0.0.1');
  const socket = net.connect(Number(process.env.CLJ_GPUI_PORT), process.env.CLJ_GPUI_HOST);
  const lines = readline.createInterface({input: socket});
  function send(value) {
    // Split even UTF-8 characters across chunks, and send blank lines too.
    const bytes = Buffer.from('\n' + JSON.stringify(value) + '\n');
    for (let i = 0; i < bytes.length; i += 3) socket.write(bytes.subarray(i, i + 3));
  }
  lines.on('line', line => {
    const msg = JSON.parse(line);
    if (msg.op === 'ready') {
      assert.equal(msg['protocol-version'], 11);
      if (mode === 'disconnect') {
        socket.end();
        setInterval(() => {}, 1000);
      } else if (mode === 'invalid') {
        socket.write('not-json\n');
      } else {
        send({op: 'render', id: 1});
      }
    } else if (msg.id === 1) {
      assert.equal(msg.ok, true);
      assert.equal(msg.tree.text, 'fresh');
      send({op: 'callback', id: 2, 'callback-id': msg.tree['on-change'], value: 'café 👩‍💻'});
    } else if (msg.id === 2) {
      assert.equal(msg.ok, true);
      send({op: 'render', id: 3});
    } else if (msg.id === 3) {
      assert.equal(msg.tree.text, 'café 👩‍💻');
      socket.end();
      process.exit(0);
    }
  });
}
