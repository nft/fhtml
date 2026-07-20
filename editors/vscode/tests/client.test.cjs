// Smoke test for extension.js in plain Node: `vscode` and
// `vscode-languageclient/node` are stubbed via the module loader, so this
// needs neither a VS Code host nor an npm install. Pins the behaviors the
// plan gates on: an untrusted workspace means no spawn and no message; a
// missing binary, a too-old binary (no `lsp` subcommand), and a server that
// dies after starting each degrade to one friendly message and no
// restart-loop; a present, new-enough binary means one client spawning
// `<bin> lsp`; plus the fhtml.path-change restart.
//
// Run: node tests/client.test.cjs  (from editors/vscode/)

"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

// ---- stubs ----------------------------------------------------------------

let settings = { path: "" };
const infoMessages = [];
const openedSettings = [];
const configListeners = [];
const trustListeners = [];
const clients = [];

// Sentinel enum values — extension.js only compares/returns them, so any
// stable identity works; the real module supplies numbers.
const ErrorAction = { Continue: "Continue", Shutdown: "Shutdown" };
const CloseAction = { DoNotRestart: "DoNotRestart", Restart: "Restart" };
const RevealOutputChannelOn = { Never: "Never" };

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
      // Simulate the user clicking the button on the first prompt, then
      // dismissing every later one.
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
    return {
      LanguageClient: LanguageClientStub,
      ErrorAction,
      CloseAction,
      RevealOutputChannelOn,
    };
  }
  return originalLoad.call(this, request, ...rest);
};

// A real, executable stand-in for an outdated compiler: it answers
// `--version` with a pre-lsp version so the probe classifies it too-old.
const oldBinary = path.join(os.tmpdir(), `fhtml-old-${process.pid}`);
fs.writeFileSync(oldBinary, '#!/bin/sh\necho "fhtml 0.1.0"\n', { mode: 0o755 });

const extension = require(path.join(__dirname, "..", "extension.js"));
const context = { subscriptions: [] };

const setPath = (p) => {
  settings = { path: p };
  return configListeners[0]({ affectsConfiguration: (s) => s === "fhtml.path" });
};

(async () => {
  try {
    // ---- an untrusted workspace: no spawn attempt, no message -------------

    settings = { path: "/nonexistent/fhtml-lsp-smoke-test" };
    vscodeStub.workspace.isTrusted = false;
    await extension.activate(context);
    assert.equal(clients.length, 0, "no client while untrusted");
    assert.equal(infoMessages.length, 0, "silent while untrusted");
    assert.equal(trustListeners.length, 1, "trust listener registered");
    assert.equal(configListeners.length, 1, "config listener registered");
    assert.equal(context.subscriptions.length, 2, "trust + config disposables");

    // ---- trust granted, missing binary: one message, settings link --------

    vscodeStub.workspace.isTrusted = true;
    await trustListeners[0]();
    assert.equal(clients.length, 0, "no client for a missing binary");
    assert.equal(infoMessages.length, 1, "one message for a missing binary");
    assert.match(infoMessages[0].message, /not found/);
    assert.match(infoMessages[0].message, /cargo install/);
    assert.match(infoMessages[0].message, /highlighting still works/);
    assert.deepEqual(openedSettings, [
      ["workbench.action.openSettings", "fhtml.path"],
    ]);

    // ---- a too-old binary: named version, upgrade hint, no client ---------

    await setPath(oldBinary);
    assert.equal(clients.length, 0, "no client for a pre-lsp binary");
    assert.equal(infoMessages.length, 2, "one message for a too-old binary");
    assert.match(infoMessages[1].message, /is 0\.1\.0/);
    assert.match(infoMessages[1].message, /needs 0\.2\.0 or newer/);
    assert.match(infoMessages[1].message, /highlighting still works/);

    // ---- a present, new-enough binary: client spawns `<bin> lsp` ----------

    // `node --version` (v25.x) reads as far newer than the 0.2.0 floor.
    await setPath(process.execPath);
    assert.equal(clients.length, 1, "one client once a good binary exists");
    const first = clients[0];
    assert.deepEqual(
      { command: first.serverOptions.command, args: first.serverOptions.args },
      { command: process.execPath, args: ["lsp"] },
    );
    assert.deepEqual(first.clientOptions.documentSelector, [
      { language: "fhtml" },
    ]);
    assert.equal(
      first.clientOptions.revealOutputChannelOn,
      RevealOutputChannelOn.Never,
      "output channel stays quiet",
    );
    assert.equal(first.started, 1);

    // ---- a server that dies: one message, no restart, no toast ------------

    const closed = first.clientOptions.errorHandler.closed();
    assert.equal(closed.action, CloseAction.DoNotRestart, "does not restart");
    assert.equal(closed.handled, true, "suppresses the default popup");
    assert.equal(infoMessages.length, 3, "one message when the server dies");
    assert.match(infoMessages[2].message, /exited unexpectedly/);
    // A second close on the same client must not re-nag.
    first.clientOptions.errorHandler.closed();
    assert.equal(infoMessages.length, 3, "the death message appears once");
    const errored = first.clientOptions.errorHandler.error();
    assert.equal(errored.action, ErrorAction.Shutdown);
    assert.equal(errored.handled, true);

    // ---- unrelated config changes never touch the client ------------------

    await configListeners[0]({ affectsConfiguration: () => false });
    assert.equal(clients.length, 1);
    assert.equal(first.stopped, 0);

    // ---- changing fhtml.path stops the old client, starts a new one -------

    await setPath(process.execPath);
    assert.equal(first.stopped, 1, "old client stopped on path change");
    assert.equal(clients.length, 2, "replacement client started");

    // ---- deactivate stops the live client, then is a no-op ----------------

    await extension.deactivate();
    assert.equal(clients[1].stopped, 1);
    assert.equal(extension.deactivate(), undefined);

    assert.equal(infoMessages.length, 3, "no further messages appeared");

    console.log("client.test.cjs: all assertions passed");
  } finally {
    fs.rmSync(oldBinary, { force: true });
  }
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
