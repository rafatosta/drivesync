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

AUTH-001 and AUTH-002 have automated tests. AUTH-003, folder selection, remote
listing and synchronization are not implemented. ACCT-003 persists sanitized
profiles under `$XDG_DATA_HOME/drivesync/accounts.json` (fallback
`$HOME/.local/share/drivesync/accounts.json`) and verifies them on startup using
the vault and Google. Tokens remain exclusively in Secret Service. A single
verified account opens Account Information; multiple accounts are listed.
Offline, locked vault or revoked credentials keep the saved profile with an
explicit error and retry/reconnect actions, never an authenticated success.
Profiles created before this index was implemented require one reconnection to
be discoverable. Reconnecting the same Google subject updates the profile.

## Maintainer: one-time Google Cloud configuration

1. Create/select the official DriveSync project in Google Cloud Console. Enable the
   **Google Drive API** for the eventual download MVP.
2. Configure Google Auth Platform branding, audience and support contact. If the
   audience is External and publishing status is Testing, add the intended test
   users. Do not use a service-account or web-application OAuth client.
3. Under Clients create **Desktop app** credentials. Download the Desktop client JSON. This is distribution metadata, not a user
   token. Supply it to the build as described below.
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

## End users

Install a configured DriveSync distribution, click **Connect Google Account**,
sign in and grant access in the external browser, then return to DriveSync.
No Google Cloud project, JSON file or environment variable is required from users.
Unlock the desktop keyring if prompted. A build without valid client configuration
explains that connection is unavailable and asks for a configured release or help
from the distributor; it never claims success.

## Maintainer: build and distribution configuration

After creating the official Desktop client once, make its downloaded JSON available
as `data/google-oauth-client.json` before building. This intentionally ignored file
is picked up by Cargo, Flatpak and GNOME Builder through the manifest's directory
source. Alternatively set `DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE` to an absolute path
visible to the build process. The build validates Desktop metadata and embeds only
`client_id` and optional `client_secret`, not arbitrary fields or endpoints.
A malformed or unreadable supplied file fails the build with a sanitized error.
Cargo tracks file changes and both build settings to prevent stale configuration.

```sh
DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE=/path/to/desktop-client.json cargo build --release --locked
# Publication gate: fail rather than ship an unconfigured binary.
DRIVESYNC_REQUIRE_GOOGLE_OAUTH=1 cargo build --release --locked
```

For Flatpak/GNOME Builder, place the file at `data/google-oauth-client.json` in the
source checkout **before** building. No runtime environment or host config mount
is needed. The development manifest allows an unconfigured build for UI work;
release packagers must set `DRIVESYNC_REQUIRE_GOOGLE_OAUTH: '1'` in the module's
`build-options.env`. When using the build file override there, stage the JSON in
the build sandbox and set its sandbox path in that same environment. Clean/rebuild
the module after changing manifest inputs. The existing manifest remains a
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
   `data/google-oauth-client.json` before building, as described above. Users of
   the resulting package do not repeat this step.
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
   With one saved account, online verification should open Account Information
   automatically. With multiple accounts, verify the saved-account list. Repeat
   offline and with a locked keyring: profiles remain, without claiming connected
   status; retry after restoring connectivity/unlocking.
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
