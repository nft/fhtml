// Smoke test for extension.js in plain Node: `vscode` and
// `vscode-languageclient/node` are stubbed via the module loader, so this
// needs neither a VS Code host nor an npm install. Pins the behaviors the
// plan gates on: an untrusted workspace means no spawn and no message, a
// missing binary means one friendly message and no client (no error spam),
// a present binary means one client spawning `<bin> lsp`; plus the
// fhtml.path-change restart.
//
// Run: node tests/client.test.cjs  (from editors/vscode/)

"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const path = require("node:path");

// ---- stubs ----------------------------------------------------------------

let settings = { path: "" };
const infoMessages = [];
const openedSettings = [];
const configListeners = [];
const trustListeners = [];
const clients = [];

const vscodeStub = {
  workspace: {
    isTrusted: true,
    getConfiguration(section) {
      assert.equal(section, "fhtml");
      return { get: (key) => settings[key] };
    },
    onDidChangeConfiguration(listener) {
      configListeners.push(listener);
      return { dispose() {} };
    },
    onDidGrantWorkspaceTrust(listener) {
      trustListeners.push(listener);
      return { dispose() {} };
    },
  },
  window: {
    async showInformationMessage(message, ...actions) {
      infoMessages.push({ message, actions });
      // Simulate the user clicking the button once, then dismissing.
      return infoMessages.length === 1 ? actions[0] : undefined;
    },
  },
  commands: {
    executeCommand(...args) {
      openedSettings.push(args);
    },
  },
};

class LanguageClientStub {
  constructor(id, name, serverOptions, clientOptions) {
    this.id = id;
    this.serverOptions = serverOptions;
    this.clientOptions = clientOptions;
    this.started = 0;
    this.stopped = 0;
    clients.push(this);
  }
  async start() {
    this.started += 1;
  }
  async stop() {
    this.stopped += 1;
  }
}

const originalLoad = Module._load;
Module._load = function (request, ...rest) {
  if (request === "vscode") {
    return vscodeStub;
  }
  if (request === "vscode-languageclient/node") {
    return { LanguageClient: LanguageClientStub };
  }
  return originalLoad.call(this, request, ...rest);
};

const extension = require(path.join(__dirname, "..", "extension.js"));
const context = { subscriptions: [] };

// ---- an untrusted workspace: no spawn attempt, no message -----------------

(async () => {
  settings = { path: "/nonexistent/fhtml-lsp-smoke-test" };
  vscodeStub.workspace.isTrusted = false;
  await extension.activate(context);
  assert.equal(clients.length, 0, "no client while untrusted");
  assert.equal(infoMessages.length, 0, "silent while untrusted");
  assert.equal(trustListeners.length, 1, "trust listener registered");

  // ---- trust granted, missing binary: one message, settings link, no client

  vscodeStub.workspace.isTrusted = true;
  await trustListeners[0]();
  assert.equal(clients.length, 0, "no client for a missing binary");
  assert.equal(infoMessages.length, 1, "exactly one friendly message");
  assert.match(infoMessages[0].message, /not found/);
  assert.match(infoMessages[0].message, /cargo install/);
  assert.match(infoMessages[0].message, /highlighting still works/);
  assert.deepEqual(openedSettings, [
    ["workbench.action.openSettings", "fhtml.path"],
  ]);
  assert.equal(configListeners.length, 1, "config listener registered");
  assert.equal(context.subscriptions.length, 2, "trust + config disposables");

  // ---- fhtml.path now points at a real executable: client starts ----------

  // Any executable that exists proves the probe logic; `node` itself is
  // guaranteed to be present for this test.
  settings = { path: process.execPath };
  await configListeners[0]({ affectsConfiguration: (s) => s === "fhtml.path" });
  assert.equal(clients.length, 1, "one client once the binary exists");
  const first = clients[0];
  assert.deepEqual(
    { command: first.serverOptions.command, args: first.serverOptions.args },
    { command: process.execPath, args: ["lsp"] },
  );
  assert.deepEqual(first.clientOptions.documentSelector, [
    { language: "fhtml" },
  ]);
  assert.equal(first.started, 1);

  // ---- unrelated config changes never touch the client --------------------

  await configListeners[0]({ affectsConfiguration: () => false });
  assert.equal(clients.length, 1);
  assert.equal(first.stopped, 0);

  // ---- changing fhtml.path stops the old client, starts a new one ---------

  await configListeners[0]({ affectsConfiguration: (s) => s === "fhtml.path" });
  assert.equal(first.stopped, 1, "old client stopped on path change");
  assert.equal(clients.length, 2, "replacement client started");

  // ---- deactivate stops the live client, then is a no-op ------------------

  await extension.deactivate();
  assert.equal(clients[1].stopped, 1);
  assert.equal(extension.deactivate(), undefined);

  // Only the missing-binary case may message the user.
  assert.equal(infoMessages.length, 1, "no other messages appeared");

  console.log("client.test.cjs: all assertions passed");
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
