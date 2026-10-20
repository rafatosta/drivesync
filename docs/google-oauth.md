# Google OAuth desktop authentication

## Implemented behavior

Welcome → Connect Google Account → system browser → loopback callback → token
exchange → HTTPS UserInfo → desktop credential vault → Account Information.
The page displays Google's stable `sub`, an optional verified email, and the
confirmed authentication state. Continue Sync Setup explains that folder setup
is still unavailable; it does not start any transfer.

One engine serves all accounts. A worker handles network, callback and vault I/O;
GTK handles only the browser launch, progress, cancellation and navigation.
`AdwNavigationView` lives inside the existing single application window.

AUTH-001 and AUTH-002 have automated tests. AUTH-003, automatic account discovery
on startup, folder selection, remote listing and synchronization are not implemented.
Credentials persist across runs; `verify_account(id)` can reuse them and renew an
expired token. The in-memory profile registry is populated by login or verification.
Opening the application again currently starts at Welcome. Reconnecting the same
Google subject updates that account instead of creating another identity.

## Google Cloud configuration

1. Create/select a development project in Google Cloud Console. Enable the
   **Google Drive API** for the eventual download MVP.
2. Configure Google Auth Platform branding, audience and support contact. If the
   audience is External and publishing status is Testing, add the intended test
   users. Do not use a service-account or web-application OAuth client.
3. Under Clients create **Desktop app** credentials. Download the JSON into a
   private local directory. Do not put the downloaded file in the repository.
4. Declare `openid`, `email`, and
   `https://www.googleapis.com/auth/drive.readonly` in Data Access as applicable.
   The application requests these scopes, offline access and an explicit account
   choice/consent. Google may omit an email; the stable subject is sufficient.
5. Desktop loopback redirects use
   `http://127.0.0.1:<ephemeral-port>/oauth/callback`. The application binds the
   port before opening the browser and sends the same URI when exchanging the
   code. Do not configure a fixed web redirect, use the retired out-of-band copy
   code flow, or expose the listener on all network interfaces.

Desktop apps are public clients: a client secret distributed with an application
cannot establish confidentiality. Some Google Desktop clients still require the
client secret issued with the client ID during token exchange; the local loader
supports the downloaded `installed.client_secret` for that compatibility. It is
never a substitute for PKCE, is never compiled into the application, and must not
be committed. No real client ID, client secret or user token is supplied here.

References: [Google desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app),
[Google OpenID Connect / UserInfo](https://developers.google.com/identity/openid-connect/openid-connect),
[RFC 7636](https://www.rfc-editor.org/rfc/rfc7636),
[RFC 8252](https://www.rfc-editor.org/rfc/rfc8252).
Reviewed on 2026-10-20.

## Scope decision for this delivery

`openid` identifies the account; `email` enables an optional verified display
address. For the download MVP's existing folders, `drive.readonly` permits reading
existing file metadata and content without write permission. `drive.file` grants
access only to files created/opened/shared with the app and does not by itself
satisfy arbitrary existing-folder discovery. `drive.metadata.readonly` does not
permit downloading file content. Full `drive` access is unnecessary here.

`drive.readonly` is a restricted scope. Public distribution may require Google's
verification and additional assessment depending on how restricted data is used.
Testing restrictions and refresh-token lifetimes depend on Google's project
status/policies. This implementation does not establish the F04 feasibility gate:
real access to an existing test folder and publication requirements remain to be
validated. See [Google's scope documentation](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).

## Local configuration

The loader checks, in order:

1. `DRIVESYNC_GOOGLE_CLIENT_ID`, with optional
   `DRIVESYNC_GOOGLE_CLIENT_SECRET` in the process environment.
2. The JSON file pointed to by `DRIVESYNC_GOOGLE_OAUTH_FILE`.
3. `$XDG_CONFIG_HOME/drivesync/oauth-client.json`, falling back to
   `$HOME/.config/drivesync/oauth-client.json` outside Flatpak.

Both the downloaded Google JSON (`{"installed": {...}}`) and a minimal object
are accepted. Example without real credentials:

```json
{
  "client_id": "YOUR_DESKTOP_CLIENT_ID.apps.googleusercontent.com"
}
```

Copy the downloaded JSON into the appropriate private config directory (mode
0600 recommended) or set the file override. Endpoints in a downloaded JSON are
not trusted: Google HTTPS authorization/token/UserInfo endpoints are fixed in
code. `.env` files are **not** loaded automatically.

Inside the development Flatpak, the default file on the host is:

```text
~/.var/app/com.rtosta.DriveSync.Devel/config/drivesync/oauth-client.json
```

Never place access or refresh tokens in that file. Tokens are serialized only
inside GNOME's Secret Service, isolated by `(application + OAuth client ID,
google:<subject>)`. A locked/unavailable vault fails the operation; there is no
plaintext or in-memory production fallback. The manifest grants only the
necessary `org.freedesktop.secrets` session-bus access for this backend. Unlock
GNOME's login keyring when prompted. Future public packaging should evaluate the
Secret portal; this development backend uses Secret Service directly.

## GNOME Builder / Flatpak test procedure

1. Open this existing project in GNOME Builder. Install GNOME Platform/SDK **50**
   and its compatible Rust SDK extension. Reload the development manifest after
   changing SDKs or permissions. No Meson project is needed.
2. Put the Desktop client JSON in the Flatpak config location above. A file
   override must point to a path actually visible in the sandbox. Environment
   configuration must be applied to the **running app**, not only build options.
3. Build and run with `flatpak/com.rtosta.DriveSync.Devel.yml`. GIO launches the
   default browser via OpenURI in the sandbox; the shared network namespace
   allows its redirect to the loopback listener. A functional desktop portal and
   unlocked Secret Service are runtime requirements.
4. Click Connect Google Account. Verify spinner, disabled Connect and enabled
   Cancel, then authorize the test user in the external browser.
5. Return to DriveSync. Check the Account Information page contains Google's real
   subject and, when granted, the verified email. Continue Sync Setup must only
   explain the pending setup functionality.
6. Go back and connect another test account: it must have another subject and
   credential entry, with the same engine. Reauthorize the first account and
   check its identity is reused.
7. Repeat with denial, Cancel, browser closed (five-minute expiry), offline mode,
   and locked/unavailable keyring. No failure should navigate to a success page.
   Cancel while the browser portal is pending should stop the worker promptly;
   HTTP requests already in flight finish within their 30-second timeout.
8. Restart and reconnect a test account. Secure storage must remain intact.
   Automatic profile restoration is deferred to F07/F08; do not interpret the
   Welcome page on startup as credential deletion.
9. Test renewal with `DriveSyncEngine::verify_account(id)` after token expiry in
   a controlled integration harness. Revoking access in Google should produce
   reauthorization-required when Google rejects the refresh. Never print tokens,
   authorization codes, browser URLs or raw provider error responses as evidence.

These are manual acceptance steps, not claims of an executed real Google login.
See [implementation evidence](auth-validation.md).

## Dependency choices and limits

- `oauth2` 5 provides random state, PKCE S256 and timing-resistant state comparison;
  its authorization builder handles URL encoding. PKCE uses a fresh verifier
  for each single-use listener. The callback rejects foreign state/host/path,
  duplicate parameters, invalid methods and oversized requests.
- `reqwest` 0.12 with Rustls provides HTTPS with 10-second connection and 30-second
  total timeouts and redirects disabled. Only the small Google token/UserInfo
  adapter uses it; no `remote` implementation is introduced. Response bodies are
  bounded to 64 KiB and errors exposed to the UI contain no provider payload.
- `keyring` 3.6 with the persistent Secret Service backend and Rust cryptography
  provides per-account storage; vendored D-Bus avoids a host development-library
  requirement. No mock backend is used by the Linux application.
- A mutex per account serializes renewal/persistence without holding a global
  mutex during network requests. Failed refresh preserves existing credentials.
- Identity comes from authenticated HTTPS UserInfo. The application never decodes
  an unverified ID token as proof of identity.
- A crash after Google grants permission but before vault persistence requires a
  new login. The UI cannot claim success before secure persistence. Remote
  revocation and destructive account removal are outside this task.
