export function safeOrigin(value) {
  const url = new URL(value);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    !(url.protocol === "https:" || (url.protocol === "http:" && local)) ||
    url.username ||
    url.password
  )
    throw new Error("Use HTTPS; HTTP only for loopback testing.");
  return url.origin;
}
export const originsOf = (binding) => [
  binding.origin,
  ...(binding.loginOrigins || []),
];
// What an AI fill depends on. Approval is recorded only from the options page, so a
// binding edited elsewhere (e.g. browser-logins.json by a local agent) is refused.
export const approvalOf = (binding) =>
  JSON.stringify([
    binding.name,
    binding.project,
    binding.usernameKey,
    binding.passwordKey,
    binding.usernameSelector || null,
    binding.passwordSelector || null,
    [...originsOf(binding)].sort(),
    [...(binding.submitOrigins || [])].sort(),
    !!binding.allowJs,
  ]);
export const approvalKey = (name) => `ai-approval:${name}`;
