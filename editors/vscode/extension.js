// fhtml VS Code client — a thin shell around `fhtml lsp`. All language
// smarts (diagnostics, formatting, symbols, definition, completion) live
// in the compiler binary; this file only finds it, spawns it, and wires
// vscode-languageclient to it. No build step: plain CommonJS, loaded as-is.

const { spawnSync } = require("node:child_process");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function binary() {
  const configured = vscode.workspace.getConfiguration("fhtml").get("path");
  return typeof configured === "string" && configured.trim()
    ? configured.trim()
    : "fhtml";
}

/** ENOENT means "not installed"; any other outcome — even a non-zero
 * exit — proves something answered at that path. */
function available(bin) {
  const probe = spawnSync(bin, ["--version"]);
  return !(probe.error && probe.error.code === "ENOENT");
}

async function start() {
  // Restricted Mode: never spawn the binary (VS Code already ignores a
  // workspace-provided fhtml.path via restrictedConfigurations, and its own
  // UI explains the reduced functionality — no message needed from us).
  if (!vscode.workspace.isTrusted) {
    return;
  }
  const bin = binary();
  if (!available(bin)) {
    // Highlighting works without the binary — say so once, quietly, and
    // don't start a client that would respawn-and-fail in a loop.
    const pick = await vscode.window.showInformationMessage(
      `fhtml: \`${bin}\` not found — diagnostics, formatting and ` +
        "go-to-definition are off (syntax highlighting still works). " +
        "Install the compiler (`cargo install --path .` in the fhtml " +
        "repo) or point the `fhtml.path` setting at the binary, then " +
        "reload the window.",
      "Open settings",
    );
    if (pick === "Open settings") {
      vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "fhtml.path",
      );
    }
    return;
  }
  client = new LanguageClient(
    "fhtml",
    "fhtml",
    { command: bin, args: ["lsp"] },
    { documentSelector: [{ language: "fhtml" }] },
  );
  await client.start();
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
