# OneKey Login privacy disclosure

OneKey Login connects only to the OneKey native host installed on this computer.
The native host contacts the OneKey server configured in the user's local profile.
The extension does not send telemetry, browsing history, credentials, or analytics
to an extension publisher. It does not load remote scripts.

Website account bindings contain website origins, project/secret names, selectors,
and user-selected authorization flags. They are stored by the CLI in the local
OneKey profile. The selected UI language is saved locally in extension storage.
Passwords are not saved in extension storage. The CLI may use its
existing encrypted offline cache under the server-selected authentication TTL.

When filling, credentials pass from the native host to the extension background
worker and directly into the authorized website's input fields. Passwords are not
returned to the popup, configuration page, or MCP. The website and privileged
browser inspection tools can still read inputs. JS login handlers may send inputs
to destinations that cannot be determined by inspecting a form; this requires a
separate user authorization.

Native messaging, scripting, activeTab and alarms are used to fill a requested
login and process authorized local AI requests. Website access is optional and
requested only when the user saves a site authorization. Removing/disabling a
binding revokes further filling; unused website permissions are removed by the
configuration page. Revocation does not clear already-filled website inputs or
log out an existing website session.

No automated submission is performed. The extension does not bypass MFA or
CAPTCHA. AI requests contain only a connection name/request ID and expire after
75 seconds. Results contain status only and are retained at most until the next
queue cleanup after expiry.
