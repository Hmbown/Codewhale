import * as vscode from "vscode";

const SECRET_KEY = "codewhale.runtimeToken";
const SETTING_KEY = "runtimeToken";

let warnedAboutWorkspaceToken = false;

/** Trim a setting value that arrived as `unknown` from a trust boundary. */
function nonEmptyString(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

/**
 * Resolve the runtime bearer token. SecretStorage is authoritative; the legacy
 * `codewhale.runtimeToken` setting is a one-way migration source only, which is
 * what `package.json`'s deprecation message and `README.md` promise.
 *
 * Only a *user-level* value is migrated. A workspace- or folder-scoped value is
 * attacker-controlled input: `codewhale.runtimeHost` is workspace-settable too,
 * so a repo-local `.vscode/settings.json` that supplied both would make merely
 * opening the repository ship a bearer token to a host of its choosing. Such a
 * value is ignored, never adopted.
 */
export async function resolveToken(context: vscode.ExtensionContext): Promise<string | undefined> {
  const stored = nonEmptyString(await context.secrets.get(SECRET_KEY));
  if (stored) {
    return stored;
  }

  const config = vscode.workspace.getConfiguration("codewhale");
  const inspected = config.inspect<string>(SETTING_KEY);
  const userToken = nonEmptyString(inspected?.globalValue);
  if (!userToken) {
    const workspaceToken =
      nonEmptyString(inspected?.workspaceValue) ??
      nonEmptyString(inspected?.workspaceFolderValue);
    if (workspaceToken && !warnedAboutWorkspaceToken) {
      warnedAboutWorkspaceToken = true;
      void vscode.window.showWarningMessage(
        "Ignoring codewhale.runtimeToken from workspace settings: a workspace cannot supply the runtime bearer token. Use CodeWhale: Set Runtime Token.",
      );
    }
    return undefined;
  }

  await context.secrets.store(SECRET_KEY, userToken);
  // One-way migration: drop the plaintext copy so it stops riding Settings Sync.
  try {
    await config.update(SETTING_KEY, undefined, vscode.ConfigurationTarget.Global);
  } catch {
    // A read-only settings.json must not cost the user a working token; the
    // secret is already stored, so keep going and leave the plaintext behind.
  }
  return userToken;
}

export async function storeToken(context: vscode.ExtensionContext, token: string): Promise<void> {
  await context.secrets.store(SECRET_KEY, token.trim());
}

export async function promptForToken(context: vscode.ExtensionContext): Promise<string | undefined> {
  const entered = await vscode.window.showInputBox({
    prompt: "Codewhale runtime bearer token (stored in VS Code secret storage)",
    password: true,
    ignoreFocusOut: true,
  });
  if (entered === undefined) {
    return undefined;
  }
  const token = entered.trim();
  if (token.length === 0) {
    await context.secrets.delete(SECRET_KEY);
    return undefined;
  }
  await storeToken(context, token);
  return token;
}
