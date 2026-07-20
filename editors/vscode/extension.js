// fhtml VS Code client — a thin shell around `fhtml lsp`. All language
// smarts (diagnostics, formatting, symbols, definition, completion) live
// in the compiler binary; this file only finds it, spawns it, and wires
// vscode-languageclient to it. No build step: plain CommonJS, loaded as-is.

const { spawnSync } = require("node:child_process");
const vscode = require("vscode");
const {
  LanguageClient,
  ErrorAction,
  CloseAction,
  RevealOutputChannelOn,
} = require("vscode-languageclient/node");

// The `fhtml lsp` subcommand landed in 0.2.0; older binaries treat `lsp`
// as a filename, exit immediately, and would surface a raw connection
// error instead of degrading gracefully.
const MIN_LSP_VERSION = [0, 2, 0];

let client;

function binary() {
  const configured = vscode.workspace.getConfiguration("fhtml").get("path");
  return typeof configured === "string" && configured.trim()
    ? configured.trim()
    : "fhtml";
}

function isOlder(a, b) {
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) {
      return a[i] < b[i];
    }
  }
  return false;
}

/** Probe `<bin> --version`. ENOENT means not installed; a version older
 * than MIN_LSP_VERSION means installed-but-no-lsp. An unreadable version
 * is treated as ok — let it try to start and let the client's error
 * handler catch a genuine failure. */
function probe(bin) {
  const result = spawnSync(bin, ["--version"], { encoding: "utf8" });
  if (result.error && result.error.code === "ENOENT") {
    return { status: "missing" };
  }
  const found = /(\d+)\.(\d+)\.(\d+)/.exec(
    `${result.stdout || ""}${result.stderr || ""}`,
  );
  if (!found) {
    return { status: "ok" };
  }
  const version = [Number(found[1]), Number(found[2]), Number(found[3])];
  return isOlder(version, MIN_LSP_VERSION)
    ? { status: "too-old", version: found[0] }
    : { status: "ok" };
}

// One quiet message pointing at the fix, plus a shortcut to the setting.
// Highlighting keeps working regardless, so this is informational.
async function offerHelp(reason) {
  const pick = await vscode.window.showInformationMessage(
    `fhtml: ${reason} — diagnostics, formatting and go-to-definition are ` +
      "off (syntax highlighting still works). Install or update the " +
      "compiler (`cargo install --git https://github.com/nft/fhtml`) or " +
      "point the `fhtml.path` setting at the binary, then reload the window.",
    "Open settings",
  );
  if (pick === "Open settings") {
    vscode.commands.executeCommand("workbench.action.openSettings", "fhtml.path");
  }
}

function startClient(bin) {
  // A binary can pass the version probe yet still fail to serve (a crash, a
  // wrong path answering --version, a broken build). Degrade to the same
  // quiet message instead of the client's raw "stream destroyed" popups,
  // and never restart-loop. `handled: true` suppresses the default toast.
  let reported = false;
  const degrade = (reason) => {
    if (!reported) {
      reported = true;
      offerHelp(reason);
    }
  };
  client = new LanguageClient(
    "fhtml",
    "fhtml",
    { command: bin, args: ["lsp"] },
    {
      documentSelector: [{ language: "fhtml" }],
      revealOutputChannelOn: RevealOutputChannelOn.Never,
      initializationFailedHandler: () => {
        degrade(`\`${bin}\` failed to start the language server`);
        return false;
      },
      errorHandler: {
        error: () => ({ action: ErrorAction.Shutdown, handled: true }),
        closed: () => {
          degrade(`the \`${bin}\` language server exited unexpectedly`);
          return { action: CloseAction.DoNotRestart, handled: true };
        },
      },
    },
  );
  client.start().catch(() => {
    degrade(`\`${bin}\` failed to start the language server`);
  });
}

async function start() {
  // Restricted Mode: never spawn the binary (VS Code already ignores a
  // workspace-provided fhtml.path via restrictedConfigurations, and its own
  // UI explains the reduced functionality — no message needed from us).
  if (!vscode.workspace.isTrusted) {
    return;
  }
  const bin = binary();
  const found = probe(bin);
  if (found.status === "missing") {
    await offerHelp(`\`${bin}\` not found`);
    return;
  }
  if (found.status === "too-old") {
    await offerHelp(
      `\`${bin}\` is ${found.version}, but the language server needs ` +
        `${MIN_LSP_VERSION.join(".")} or newer`,
    );
    return;
  }
  startClient(bin);
}

async function activate(context) {
  context.subscriptions.push(
    vscode.workspace.onDidGrantWorkspaceTrust(() => start()),
    vscode.workspace.onDidChangeConfiguration(async (e) => {
      if (!e.affectsConfiguration("fhtml.path")) {
        return;
      }
      if (client) {
        await client.stop();
        client = undefined;
      }
      await start();
    }),
  );
  await start();
}

function deactivate() {
  if (!client) {
    return undefined;
  }
  const stopping = client.stop();
  client = undefined;
  return stopping;
}

module.exports = { activate, deactivate };
