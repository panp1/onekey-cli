import { originsOf } from "./policy.js";

export const ALL_HTTPS = "https://*/*";

// Browser permissions are the source of truth, including changes in Chrome/Edge settings.
export const hasAllHttpsAccess = () =>
  chrome.permissions.contains({ origins: [ALL_HTTPS] });

// Called directly from the checkbox gesture so Chrome can show its permission prompt.
export const setAllHttpsAccess = (enabled) =>
  enabled
    ? chrome.permissions.request({ origins: [ALL_HTTPS] })
    : chrome.permissions.remove({ origins: [ALL_HTTPS] });

export async function removeUnusedHostPermissions(bindings) {
  const permissions = await chrome.permissions.getAll();
  const used = new Set(
    bindings
      .filter((binding) => binding.enabled)
      .flatMap((binding) => originsOf(binding).map((origin) => `${origin}/*`)),
  );
  // A deliberate global grant is independent of individual account bindings.
  const unused = (permissions.origins || []).filter(
    (origin) => origin !== ALL_HTTPS && !used.has(origin),
  );
  if (unused.length) await chrome.permissions.remove({ origins: unused });
}
