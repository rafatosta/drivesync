# Google OAuth desktop authentication

## Implemented behavior

Welcome → Connect Google Account → system browser → loopback callback → token
exchange → HTTPS UserInfo → desktop credential vault → folder setup → dashboard.
Configured accounts open Dashboard on startup. Connection failures preserve saved
settings and are shown explicitly; no file transfers start in this increment.

One engine serves all accounts. A worker handles network, callback and vault I/O;
GTK handles only the browser launch, progress, cancellation and navigation.
`AdwNavigationView` lives inside the existing single application window.

AUTH-001/002 and account restoration have automated tests. Startup discovers saved
profiles and verifies them through the vault and Google; failures preserve profiles
with retry/reconnect actions. Older accounts need one reconnection to register a
profile. Storage and restoration rules: [ACCT-003](../engine/src/accounts/algorithms.md#acct-003--persistir-e-restaurar-perfis-na-abertura).
Folder selection uses read-only Drive folder metadata; file synchronization and
AUTH-003 remain unimplemented.

## Maintainer: one-time Google Cloud configuration

1. Create/select the official DriveSync project in Google Cloud Console. Enable the
   **Google Drive API** for the eventual download MVP.
2. Configure Google Auth Platform branding, audience and support contact. If the
   audience is External and publishing status is Testing, add the intended test
   users. Do not use a service-account or web-application OAuth client.
3. Under Clients create **Desktop app** credentials. Download the Desktop client JSON. This is distribution metadata, not a user
   token. Supply it to the build as described below.
4. Declare `openid`, `email`, and
   `https://www.googleapis.com/auth/drive` in Data Access as applicable.
   The application requests these scopes, offline access and an explicit account
   choice/consent at account connection. Google may omit an email; the stable
   subject is sufficient. Upload remains disabled until the user opts in.
5. Desktop loopback redirects use
   `http://127.0.0.1:<ephemeral-port>/oauth/callback`. The application binds the
   port before opening the browser and sends the same URI when exchanging the
   code. Do not configure a fixed web redirect, use the retired out-of-band copy
   code flow, or expose the listener on all network interfaces.

Desktop apps are public clients: a client secret distributed with an application
cannot establish confidentiality. Some Google Desktop clients still require the
client secret issued with the client ID during token exchange; the build and runtime loaders
support the downloaded `installed.client_secret` for that compatibility. It is
never a substitute for PKCE, may be compiled into the application as public compatibility metadata. It must
never be described as confidential. No official client metadata or user token
is supplied in this checkout.

References: [Google desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app),
[Google OpenID Connect / UserInfo](https://developers.google.com/identity/openid-connect/openid-connect),
[RFC 7636](https://www.rfc-editor.org/rfc/rfc7636),
[RFC 8252](https://www.rfc-editor.org/rfc/rfc8252).
Reviewed on 2026-10-20.

## Scope decision for this delivery

`openid` identifies the account; `email` enables an optional verified display
address. The DriveSync requests the full `drive` scope at initial account
connection because the implemented features read existing files and may create
new files when the user explicitly enables upload. The upload preference remains
off by default; disabling it prevents DriveSync from creating local-file upload
operations, but does not narrow the OAuth grant. `drive.file` does not by itself
satisfy discovery of arbitrary existing folders.

`drive` is a restricted scope and grants broader access than download-only use.
Public distribution may require Google's verification and additional assessment
depending on how restricted data is used. Testing restrictions and refresh-token
lifetimes depend on Google's project status/policies. Real access to an existing
test folder and publication requirements remain to be validated. See
[Google's scope documentation](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).

## End users

Install a configured DriveSync distribution, click **Connect Google Account**,
sign in and grant access in the external browser, then return to DriveSync.
No Google Cloud project, JSON file or environment variable is required from users.
Unlock the desktop keyring if prompted. A build without valid client configuration
explains that connection is unavailable and asks for a configured release or help
from the distributor; it never claims success.

## Development and production packaging

Development uses `com.rtosta.DriveSync.Devel` and the ignored testing client
`data/oauth/development.json`.
Production uses `com.rtosta.DriveSync` and requires a separate production
Desktop OAuth client staged as `data/oauth/production.json` before building.
The production Flatpak manifest fails closed if that file is missing.
Both packages build the same Rust application but have distinct desktop
identities and Google OAuth projects. See [build profiles](build-profiles.md).

## Maintainer: build and distribution configuration

After creating the official Desktop client once, make its downloaded JSON available
as `data/oauth/development.json` before building. This intentionally ignored file
is read at compile time and embedded in the binary; it is not opened at runtime.
The Flatpak manifest's directory source includes local ignored files. Visibility
in Builder's project tree does not prove whether a file reached the build.
Alternatively set `DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE` to an absolute path
visible to the build process. Relative overrides resolve from the workspace root
(the directory containing the top-level Cargo.toml), including in Flatpak.
The build validates Desktop metadata and embeds only
`client_id` and optional `client_secret`, not arbitrary fields or endpoints.
A malformed or unreadable supplied file fails the build with a sanitized error.
Cargo tracks file changes and both build settings to prevent stale configuration.

```sh
DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE=/path/to/desktop-client.json cargo build --release --locked
# Publication gate: fail rather than ship an unconfigured binary.
DRIVESYNC_REQUIRE_GOOGLE_OAUTH=1 cargo build --release --locked
```

For Flatpak/GNOME Builder, place the testing Desktop OAuth file at
`data/oauth/development.json` in the
source checkout **before** building. No runtime environment or host config mount
is needed. The development manifest allows an unconfigured build for UI work;
release packagers must set `DRIVESYNC_REQUIRE_GOOGLE_OAUTH: '1'` in the module's
`build-options.env`. When using the build file override there, stage the JSON in
the build sandbox and set its sandbox path in that same environment. Clean/rebuild
the module after changing manifest inputs or adding the JSON to an existing build.
In Builder, use Clean/Rebuild and verify the active manifest is the development
manifest. The build emits a warning when it embeds no client. Do not set
`DRIVESYNC_GOOGLE_OAUTH_FILE` to a host checkout path at runtime: that override
takes precedence over embedded metadata and may be inaccessible in the sandbox.
The existing manifest remains a
network-enabled development build, not a reproducible release manifest.

The maintainer must also configure consent branding, audience, support and declared
scopes, enable Drive API, and satisfy Google's publication/verification requirements
before broad distribution. Testing-mode accounts must be explicitly allowed.
Changing to a different client ID changes the vault namespace and requires users
to authorize again. Reusing one official client across releases avoids this.

## Optional development runtime overrides

Priority is explicit `DRIVESYNC_GOOGLE_CLIENT_ID` (with optional
`DRIVESYNC_GOOGLE_CLIENT_SECRET`), then explicit `DRIVESYNC_GOOGLE_OAUTH_FILE`, then
the embedded distribution client. A secret without an ID is rejected; a bad
explicit override fails closed rather than silently choosing another identity.
The former implicit XDG config file is no longer read, preventing stale local
configuration from overriding the distribution. Developers using that file can
set the file override explicitly. `.env` files are not loaded automatically.
Both downloaded `installed` JSON and minimal `client_id`/`client_secret` objects
are supported; web-client JSON is rejected. Google endpoints remain fixed in code.

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
2. As maintainer, stage the official Desktop JSON in
   `data/oauth/development.json` before building, as described above. Users of
   the resulting package do not repeat this step.
3. Build and run with `flatpak/com.rtosta.DriveSync.Devel.yml`. GIO launches the
   default browser via OpenURI in the sandbox; the shared network namespace
   allows its redirect to the loopback listener. A functional desktop portal and
   unlocked Secret Service are runtime requirements.
4. Click Connect Google Account. Verify spinner, disabled Connect and enabled
   Cancel, then authorize the test user in the external browser.
5. Return to DriveSync. Check the folder setup page identifies the connected account. Choose remote
   and local folders, save, and verify the Dashboard shows them without claiming
   that file transfers have started.
6. Go back and connect another test account: it must have another subject and
   credential entry, with the same engine. Reauthorize the first account and
   check its identity is reused.
7. Repeat with denial, Cancel, browser closed (five-minute expiry), offline mode,
   and locked/unavailable keyring. No failure should navigate to a success page.
   Cancel while the browser portal is pending should stop the worker promptly;
   HTTP requests already in flight finish within their 30-second timeout.
8. After registering an account, Quit/Ctrl+Q and reopen. One verified account
   with a saved folder should open Dashboard; an unconfigured account should open
   folder selection.
   Repeat offline and with a locked keyring: preserve profiles without claiming
   authentication; retry after recovery.
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
